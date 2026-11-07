//! Mixed RGB (`A8 0B` to `0F`): each LED belongs to a region, and each region cycles through its
//! own list of lighting effects. From `keyboards/keychron/common/rgb/keychron_rgb.c` and
//! `mixed_rgb.c` at Keychron/qmk_firmware `2b493ff9`. See `docs/protocol/keychron.md`,
//! "Keychron RGB group" and "Mixed RGB".
//!
//! A set changes RAM only. [`MixedRgb::save`] writes it to EEPROM together with every other
//! Keychron RGB setting. Regions and effects show only while the lighting effect is Keychron's
//! mixed RGB effect.
//!
//! Hardware verification is pending: the layouts come from source only (#20).

use openkeychron_hid::HidTransport;

use crate::client::{Keychron, KeychronError, RGB_GROUP};
use crate::perkey::{MatrixSize, SAVE};

const GET_INFO: u8 = 0x0B;
const GET_REGIONS: u8 = 0x0C;
const SET_REGIONS: u8 = 0x0D;
const GET_EFFECT_LIST: u8 = 0x0E;
const SET_EFFECT_LIST: u8 = 0x0F;

/// The firmware's limits on regions per get and set request: what fits in one report.
const REGIONS_PER_GET: u8 = 29;
const REGIONS_PER_SET: u8 = 28;

/// The firmware's limit on effects per get or set request.
const EFFECTS_PER_REQUEST: u8 = 3;

/// `EFFECT_DATA_LEN`: effect, H, S, speed, then the duration as a little-endian u32.
const EFFECT_LEN: usize = 8;

const FEATURE: &str = "mixed RGB (Keychron firmware 1.1.0 or later)";

/// One entry of a region's effect list (`effect_config_t`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MixedEffect {
    /// The RGB matrix effect number, which depends on the firmware build. `0` is an empty slot:
    /// the firmware skips it. It refuses numbers from its mixed RGB effect up.
    pub effect: u8,
    pub hue: u8,
    pub saturation: u8,
    pub speed: u8,
    /// How long the effect plays before the region moves to its next non-empty slot.
    pub duration_ms: u32,
}

impl MixedEffect {
    fn to_bytes(self) -> [u8; EFFECT_LEN] {
        let [d0, d1, d2, d3] = self.duration_ms.to_le_bytes();
        [
            self.effect,
            self.hue,
            self.saturation,
            self.speed,
            d0,
            d1,
            d2,
            d3,
        ]
    }

    fn from_bytes([effect, hue, saturation, speed, d0, d1, d2, d3]: [u8; EFFECT_LEN]) -> Self {
        Self {
            effect,
            hue,
            saturation,
            speed,
            duration_ms: u32::from_le_bytes([d0, d1, d2, d3]),
        }
    }
}

/// Mixed RGB on a firmware that was checked to support it. Holds the LED count and the
/// keyboard's region layout, so every request stays inside the firmware's arrays: its effect
/// list handlers accept a region one past the last (`region > EFFECT_LAYERS`).
#[derive(Debug)]
pub struct MixedRgb<'keychron, T> {
    keychron: &'keychron mut Keychron<T>,
    led_count: u8,
    region_count: u8,
    effects_per_region: u8,
}

impl<T: HidTransport> Keychron<T> {
    /// Checks the Keychron RGB feature bit (`0xA2`) and protocol version (`A8 01`), then reads
    /// the LED count (`A8 05`) and the region layout (`A8 0B`). Fails with
    /// [`KeychronError::Unsupported`] on firmware without mixed RGB, before anything is changed.
    pub fn mixed_rgb(&mut self) -> Result<MixedRgb<'_, T>, KeychronError> {
        let led_count = self.keychron_rgb_led_count(FEATURE)?;
        let info = self.group(RGB_GROUP, GET_INFO, &[])?;
        let (region_count, effects_per_region) = (info[3], info[4]);
        if region_count == 0 || effects_per_region == 0 {
            return Err(KeychronError::Invalid(format!(
                "the keyboard reports {region_count} mixed RGB regions of \
                 {effects_per_region} effects; expected at least one of each"
            )));
        }
        Ok(MixedRgb {
            keychron: self,
            led_count,
            region_count,
            effects_per_region,
        })
    }
}

impl<T: HidTransport> MixedRgb<'_, T> {
    #[must_use]
    pub const fn led_count(&self) -> u8 {
        self.led_count
    }

    #[must_use]
    pub const fn region_count(&self) -> u8 {
        self.region_count
    }

    #[must_use]
    pub const fn effects_per_region(&self) -> u8 {
        self.effects_per_region
    }

    /// `A8 06` for every column of `row`: the first LED under each, `None` for a column
    /// without one. Refuses a row outside `matrix` without sending.
    pub fn leds_in_row(
        &mut self,
        matrix: MatrixSize,
        row: u8,
    ) -> Result<Vec<Option<u8>>, KeychronError> {
        let every_column = 1u32
            .checked_shl(u32::from(matrix.columns))
            .map_or(u32::MAX, |bit| bit - 1);
        self.keychron
            .leds_under(self.led_count, matrix, row, every_column)
    }

    /// `A8 0C` for every LED, in requests of at most 29: the region each LED belongs to.
    pub fn regions(&mut self) -> Result<Vec<u8>, KeychronError> {
        self.read_regions(0, self.led_count)
    }

    /// Writes: `A8 0D` for the LEDs from `start`, in requests of at most 28, reading each request
    /// back. Changes RAM only: call [`MixedRgb::save`] to keep it after a power cycle. Refuses
    /// LEDs or regions that do not exist without sending; a mismatch after an earlier request
    /// leaves that request's LEDs changed.
    pub fn set_regions(&mut self, start: u8, regions: &[u8]) -> Result<(), KeychronError> {
        let end = end_within(start, regions.len(), self.led_count).ok_or_else(|| {
            KeychronError::Invalid(format!(
                "{} LEDs from LED {start} do not fit; this keyboard has {}",
                regions.len(),
                self.led_count
            ))
        })?;
        if let Some(region) = regions.iter().find(|&&region| region >= self.region_count) {
            return Err(self.missing_region(*region));
        }
        let mut led = start;
        while led < end {
            let count = (end - led).min(REGIONS_PER_SET);
            let batch = &regions[usize::from(led - start)..][..usize::from(count)];
            let mut request = vec![led, count];
            request.extend_from_slice(batch);
            self.keychron.group(RGB_GROUP, SET_REGIONS, &request)?;
            let read = self.read_regions(led, count)?;
            if read != batch {
                return Err(KeychronError::ReadBackMismatch {
                    written: format!("regions of LEDs {led}..: {batch:?}"),
                    read: format!("regions of LEDs {led}..: {read:?}"),
                });
            }
            led += count;
        }
        Ok(())
    }

    /// `A8 0E` for every slot of `region`'s effect list, in requests of at most three.
    pub fn effects(&mut self, region: u8) -> Result<Vec<MixedEffect>, KeychronError> {
        self.require_region(region)?;
        self.read_effects(region, 0, self.effects_per_region)
    }

    /// Writes: `A8 0F` for `region`'s effect slots from `start`, in requests of at most three,
    /// reading each request back. Changes RAM only: call [`MixedRgb::save`] to keep it after a
    /// power cycle. Refuses regions or slots that do not exist without sending; a mismatch after
    /// an earlier request leaves that request's slots changed.
    pub fn set_effects(
        &mut self,
        region: u8,
        start: u8,
        effects: &[MixedEffect],
    ) -> Result<(), KeychronError> {
        self.require_region(region)?;
        let end = end_within(start, effects.len(), self.effects_per_region).ok_or_else(|| {
            KeychronError::Invalid(format!(
                "{} effects from slot {start} do not fit; each region has {} slots",
                effects.len(),
                self.effects_per_region
            ))
        })?;
        let mut slot = start;
        while slot < end {
            let count = (end - slot).min(EFFECTS_PER_REQUEST);
            let batch = &effects[usize::from(slot - start)..][..usize::from(count)];
            let mut request = vec![region, slot, count];
            request.extend(batch.iter().flat_map(|effect| effect.to_bytes()));
            self.keychron.group(RGB_GROUP, SET_EFFECT_LIST, &request)?;
            let read = self.read_effects(region, slot, count)?;
            if read != batch {
                return Err(KeychronError::ReadBackMismatch {
                    written: format!("region {region} effects from slot {slot}: {batch:?}"),
                    read: format!("region {region} effects from slot {slot}: {read:?}"),
                });
            }
            slot += count;
        }
        Ok(())
    }

    /// Writes: `A8 02` stores all Keychron RGB settings in EEPROM at once: indicators, per-key
    /// type and colours, mixed regions and effects, including changes another program left in
    /// RAM.
    pub fn save(&mut self) -> Result<(), KeychronError> {
        self.keychron.group(RGB_GROUP, SAVE, &[])?;
        Ok(())
    }

    /// `A8 0C` for `count` LEDs from `start`, already checked against the LED count. Fails on a
    /// region the keyboard did not report, rather than handing it to a later write.
    fn read_regions(&mut self, start: u8, count: u8) -> Result<Vec<u8>, KeychronError> {
        let mut regions = Vec::with_capacity(usize::from(count));
        let end = start + count;
        let mut led = start;
        while led < end {
            let batch = (end - led).min(REGIONS_PER_GET);
            let reply = self.keychron.group(RGB_GROUP, GET_REGIONS, &[led, batch])?;
            regions.extend_from_slice(&reply[3..3 + usize::from(batch)]);
            led += batch;
        }
        if let Some(region) = regions.iter().find(|&&region| region >= self.region_count) {
            return Err(KeychronError::Invalid(format!(
                "the keyboard puts an LED in mixed RGB region {region}, past its {} regions",
                self.region_count
            )));
        }
        Ok(regions)
    }

    /// `A8 0E` for `count` slots of `region` from `start`, already checked against the layout.
    fn read_effects(
        &mut self,
        region: u8,
        start: u8,
        count: u8,
    ) -> Result<Vec<MixedEffect>, KeychronError> {
        let mut effects = Vec::with_capacity(usize::from(count));
        let end = start + count;
        let mut slot = start;
        while slot < end {
            let batch = (end - slot).min(EFFECTS_PER_REQUEST);
            let reply = self
                .keychron
                .group(RGB_GROUP, GET_EFFECT_LIST, &[region, slot, batch])?;
            let (entries, _) = reply[3..].as_chunks::<EFFECT_LEN>();
            effects.extend(
                entries
                    .iter()
                    .take(usize::from(batch))
                    .map(|&bytes| MixedEffect::from_bytes(bytes)),
            );
            slot += batch;
        }
        Ok(effects)
    }

    fn require_region(&self, region: u8) -> Result<(), KeychronError> {
        if region < self.region_count {
            Ok(())
        } else {
            Err(self.missing_region(region))
        }
    }

    fn missing_region(&self, region: u8) -> KeychronError {
        KeychronError::Invalid(format!(
            "mixed RGB region {region} does not exist; this keyboard has {}",
            self.region_count
        ))
    }
}

/// The end of `length` entries from `start`, when they all fit below `limit`.
fn end_within(start: u8, length: usize, limit: u8) -> Option<u8> {
    let end = usize::from(start) + length;
    u8::try_from(end).ok().filter(|&end| end <= limit)
}

#[cfg(test)]
mod tests {
    use openkeychron_hid::{DeviceIdentity, Event, Fixture, ReplayTransport};

    use super::MixedEffect;
    use crate::client::{Keychron, KeychronError, REPORT_LEN};
    use crate::perkey::MatrixSize;

    const V5_MAX: MatrixSize = MatrixSize {
        rows: 6,
        columns: 19,
    };

    /// `0xA2` reply with the V5 Max's feature bits: 0, 1, 2, 4, 5, 6, 7, 8 and 9.
    const FEATURES: [u8; 4] = [0xa2, 0x00, 0b1111_0111, 0b0000_0011];

    fn report(prefix: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0u8; REPORT_LEN];
        bytes[..prefix.len()].copy_from_slice(prefix);
        bytes
    }

    fn exchange(request: &[u8], reply: &[u8]) -> Vec<Event> {
        vec![
            Event::Write {
                data: report(request),
            },
            Event::Read {
                data: report(reply),
            },
        ]
    }

    /// The support checks, LED count and mixed-effect info that open every mixed RGB session.
    fn opening(led_count: u8, regions: u8, effects_per_region: u8) -> Vec<Event> {
        [
            exchange(&[0xa2], &FEATURES),
            exchange(&[0xa8, 0x01], &[0xa8, 0x01, 0x00, 0x01, 0x00]),
            exchange(&[0xa8, 0x05], &[0xa8, 0x05, 0x00, led_count]),
            exchange(
                &[0xa8, 0x0b],
                &[0xa8, 0x0b, 0x00, regions, effects_per_region],
            ),
        ]
        .concat()
    }

    /// The V5 Max ISO: 98 LEDs, 2 regions of 5 effects.
    fn v5_max_opening() -> Vec<Event> {
        opening(98, 2, 5)
    }

    fn keychron(sessions: &[Vec<Event>]) -> Keychron<ReplayTransport> {
        let device = DeviceIdentity {
            name: "synthetic".to_owned(),
            vendor_id: 0x3434,
            product_id: 0x0951,
            usage_page: 0xff60,
            usage: 0x61,
        };
        Keychron::new(ReplayTransport::new(Fixture::new(
            device,
            sessions.concat(),
        )))
    }

    /// Firmware default for region 1's first effect: effect 2, red, mid speed, 5 s.
    const DEFAULT_SECOND_REGION: MixedEffect = MixedEffect {
        effect: 2,
        hue: 0,
        saturation: 255,
        speed: 127,
        duration_ms: 5000,
    };

    const EMPTY: MixedEffect = MixedEffect {
        effect: 0,
        hue: 0,
        saturation: 0,
        speed: 0,
        duration_ms: 0,
    };

    /// 8 bytes per effect: effect, H, S, speed, then the duration as a little-endian u32.
    fn effect_bytes(effect: MixedEffect) -> Vec<u8> {
        let mut bytes = vec![effect.effect, effect.hue, effect.saturation, effect.speed];
        bytes.extend(effect.duration_ms.to_le_bytes());
        bytes
    }

    fn reply(prefix: &[u8], data: &[u8]) -> Vec<u8> {
        let mut bytes = prefix.to_vec();
        bytes.extend_from_slice(data);
        bytes
    }

    #[test]
    fn reads_the_region_layout_after_checking_support() {
        let mut keychron = keychron(&[v5_max_opening()]);
        assert_eq!(
            keychron.mixed_rgb().map(|rgb| (
                rgb.led_count(),
                rgb.region_count(),
                rgb.effects_per_region()
            )),
            Ok((98, 2, 5))
        );
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn says_unsupported_when_the_keychron_rgb_feature_bit_is_clear() {
        let mut keychron = keychron(&[exchange(&[0xa2], &[0xa2, 0x00, 0b0111_0111, 0x03])]);
        assert_eq!(
            keychron.mixed_rgb().map(|rgb| rgb.region_count()),
            Err(KeychronError::Unsupported(
                "mixed RGB (Keychron firmware 1.1.0 or later)"
            ))
        );
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn says_unsupported_when_the_firmware_has_no_feature_command() {
        let mut keychron = keychron(&[exchange(&[0xa2], &[0xff])]);
        assert_eq!(
            keychron.mixed_rgb().map(|rgb| rgb.region_count()),
            Err(KeychronError::Unsupported(
                "mixed RGB (Keychron firmware 1.1.0 or later)"
            ))
        );
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn refuses_an_unknown_rgb_protocol_version() {
        let mut keychron = keychron(&[
            exchange(&[0xa2], &FEATURES),
            exchange(&[0xa8, 0x01], &[0xa8, 0x01, 0x00, 0x02, 0x00]),
        ]);
        assert_eq!(
            keychron.mixed_rgb().map(|rgb| rgb.region_count()),
            Err(KeychronError::UnknownRgbProtocol(2))
        );
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn rejects_a_keyboard_reporting_no_regions() {
        let mut keychron = keychron(&[opening(98, 0, 5)]);
        assert!(matches!(
            keychron.mixed_rgb().map(|rgb| rgb.region_count()),
            Err(KeychronError::Invalid(_))
        ));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn rejects_a_keyboard_reporting_no_effects_per_region() {
        let mut keychron = keychron(&[opening(98, 2, 0)]);
        assert!(matches!(
            keychron.mixed_rgb().map(|rgb| rgb.region_count()),
            Err(KeychronError::Invalid(_))
        ));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn reads_every_leds_region_in_batches_of_twenty_nine() {
        let mut first = vec![0u8; 29];
        first[3] = 1;
        let mut keychron = keychron(&[
            opening(31, 2, 5),
            exchange(&[0xa8, 0x0c, 0, 29], &reply(&[0xa8, 0x0c, 0x00], &first)),
            exchange(&[0xa8, 0x0c, 29, 2], &[0xa8, 0x0c, 0x00, 1, 0]),
        ]);
        let mut expected = first.clone();
        expected.extend([1, 0]);
        assert_eq!(keychron.mixed_rgb().unwrap().regions(), Ok(expected));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn rejects_a_region_read_past_the_region_count() {
        let mut keychron = keychron(&[
            opening(3, 2, 5),
            exchange(&[0xa8, 0x0c, 0, 3], &[0xa8, 0x0c, 0x00, 0, 2, 1]),
        ]);
        assert!(matches!(
            keychron.mixed_rgb().unwrap().regions(),
            Err(KeychronError::Invalid(_))
        ));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn sets_regions_and_reads_them_back() {
        let mut keychron = keychron(&[
            v5_max_opening(),
            exchange(&[0xa8, 0x0d, 5, 3, 1, 1, 0], &[0xa8, 0x0d, 0x00]),
            exchange(&[0xa8, 0x0c, 5, 3], &[0xa8, 0x0c, 0x00, 1, 1, 0]),
        ]);
        assert_eq!(
            keychron.mixed_rgb().unwrap().set_regions(5, &[1, 1, 0]),
            Ok(())
        );
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn sets_regions_in_batches_of_twenty_eight() {
        let regions = [1u8; 30];
        let mut keychron = keychron(&[
            v5_max_opening(),
            exchange(&reply(&[0xa8, 0x0d, 0, 28], &[1; 28]), &[0xa8, 0x0d, 0x00]),
            exchange(&[0xa8, 0x0c, 0, 28], &reply(&[0xa8, 0x0c, 0x00], &[1; 28])),
            exchange(&[0xa8, 0x0d, 28, 2, 1, 1], &[0xa8, 0x0d, 0x00]),
            exchange(&[0xa8, 0x0c, 28, 2], &[0xa8, 0x0c, 0x00, 1, 1]),
        ]);
        assert_eq!(
            keychron.mixed_rgb().unwrap().set_regions(0, &regions),
            Ok(())
        );
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn fails_when_regions_read_back_differently() {
        let mut keychron = keychron(&[
            v5_max_opening(),
            exchange(&[0xa8, 0x0d, 5, 1, 1], &[0xa8, 0x0d, 0x00]),
            exchange(&[0xa8, 0x0c, 5, 1], &[0xa8, 0x0c, 0x00, 0]),
        ]);
        assert!(matches!(
            keychron.mixed_rgb().unwrap().set_regions(5, &[1]),
            Err(KeychronError::ReadBackMismatch { .. })
        ));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn refuses_regions_past_the_led_count_without_sending() {
        let mut keychron = keychron(&[v5_max_opening()]);
        assert!(matches!(
            keychron.mixed_rgb().unwrap().set_regions(97, &[1, 1]),
            Err(KeychronError::Invalid(_))
        ));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn refuses_a_region_number_past_the_region_count_without_sending() {
        let mut keychron = keychron(&[v5_max_opening()]);
        assert!(matches!(
            keychron.mixed_rgb().unwrap().set_regions(0, &[0, 2]),
            Err(KeychronError::Invalid(_))
        ));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn reads_a_regions_effect_list_in_batches_of_three() {
        let first = [DEFAULT_SECOND_REGION, EMPTY, EMPTY]
            .into_iter()
            .flat_map(effect_bytes)
            .collect::<Vec<_>>();
        let second = [EMPTY, EMPTY]
            .into_iter()
            .flat_map(effect_bytes)
            .collect::<Vec<_>>();
        let mut keychron = keychron(&[
            v5_max_opening(),
            exchange(&[0xa8, 0x0e, 1, 0, 3], &reply(&[0xa8, 0x0e, 0x00], &first)),
            exchange(&[0xa8, 0x0e, 1, 3, 2], &reply(&[0xa8, 0x0e, 0x00], &second)),
        ]);
        assert_eq!(
            keychron.mixed_rgb().unwrap().effects(1),
            Ok(vec![DEFAULT_SECOND_REGION, EMPTY, EMPTY, EMPTY, EMPTY])
        );
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn refuses_to_read_the_region_the_firmware_check_lets_through() {
        let mut keychron = keychron(&[v5_max_opening()]);
        assert!(matches!(
            keychron.mixed_rgb().unwrap().effects(2),
            Err(KeychronError::Invalid(_))
        ));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn sets_an_effect_and_reads_it_back() {
        let mut keychron = keychron(&[
            v5_max_opening(),
            exchange(
                &reply(&[0xa8, 0x0f, 1, 2, 1], &effect_bytes(DEFAULT_SECOND_REGION)),
                &[0xa8, 0x0f, 0x00],
            ),
            exchange(
                &[0xa8, 0x0e, 1, 2, 1],
                &reply(&[0xa8, 0x0e, 0x00], &effect_bytes(DEFAULT_SECOND_REGION)),
            ),
        ]);
        assert_eq!(
            keychron
                .mixed_rgb()
                .unwrap()
                .set_effects(1, 2, &[DEFAULT_SECOND_REGION]),
            Ok(())
        );
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn sets_effects_in_batches_of_three() {
        let effects = [DEFAULT_SECOND_REGION; 4];
        let three = effects[..3]
            .iter()
            .copied()
            .flat_map(effect_bytes)
            .collect::<Vec<_>>();
        let one = effect_bytes(DEFAULT_SECOND_REGION);
        let mut keychron = keychron(&[
            v5_max_opening(),
            exchange(&reply(&[0xa8, 0x0f, 0, 0, 3], &three), &[0xa8, 0x0f, 0x00]),
            exchange(&[0xa8, 0x0e, 0, 0, 3], &reply(&[0xa8, 0x0e, 0x00], &three)),
            exchange(&reply(&[0xa8, 0x0f, 0, 3, 1], &one), &[0xa8, 0x0f, 0x00]),
            exchange(&[0xa8, 0x0e, 0, 3, 1], &reply(&[0xa8, 0x0e, 0x00], &one)),
        ]);
        assert_eq!(
            keychron.mixed_rgb().unwrap().set_effects(0, 0, &effects),
            Ok(())
        );
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn fails_when_an_effect_reads_back_differently() {
        let mut keychron = keychron(&[
            v5_max_opening(),
            exchange(
                &reply(&[0xa8, 0x0f, 1, 0, 1], &effect_bytes(DEFAULT_SECOND_REGION)),
                &[0xa8, 0x0f, 0x00],
            ),
            exchange(
                &[0xa8, 0x0e, 1, 0, 1],
                &reply(&[0xa8, 0x0e, 0x00], &effect_bytes(EMPTY)),
            ),
        ]);
        assert!(matches!(
            keychron
                .mixed_rgb()
                .unwrap()
                .set_effects(1, 0, &[DEFAULT_SECOND_REGION]),
            Err(KeychronError::ReadBackMismatch { .. })
        ));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn refuses_to_set_the_region_the_firmware_check_lets_through() {
        let mut keychron = keychron(&[v5_max_opening()]);
        assert!(matches!(
            keychron
                .mixed_rgb()
                .unwrap()
                .set_effects(2, 0, &[DEFAULT_SECOND_REGION]),
            Err(KeychronError::Invalid(_))
        ));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn refuses_effects_past_the_effect_list_without_sending() {
        let mut keychron = keychron(&[v5_max_opening()]);
        assert!(matches!(
            keychron
                .mixed_rgb()
                .unwrap()
                .set_effects(0, 4, &[EMPTY, EMPTY]),
            Err(KeychronError::Invalid(_))
        ));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn reports_an_effect_the_firmware_refuses() {
        let unknown = MixedEffect {
            effect: 200,
            ..DEFAULT_SECOND_REGION
        };
        let mut keychron = keychron(&[
            v5_max_opening(),
            exchange(
                &reply(&[0xa8, 0x0f, 0, 0, 1], &effect_bytes(unknown)),
                &[0xa8, 0x0f, 0x01],
            ),
        ]);
        assert_eq!(
            keychron.mixed_rgb().unwrap().set_effects(0, 0, &[unknown]),
            Err(KeychronError::Refused("0xa8 0x0f".to_owned()))
        );
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn reads_the_leds_of_a_whole_matrix_row_in_one_request() {
        let mut leds = [0xffu8; 19];
        leds[0] = 16;
        leds[2] = 17;
        let mut keychron = keychron(&[
            v5_max_opening(),
            exchange(
                &[0xa8, 0x06, 1, 0xff, 0xff, 0x07],
                &reply(&[0xa8, 0x06, 0x00], &leds),
            ),
        ]);
        let mut expected = vec![None; 19];
        expected[0] = Some(16);
        expected[2] = Some(17);
        assert_eq!(
            keychron.mixed_rgb().unwrap().leds_in_row(V5_MAX, 1),
            Ok(expected)
        );
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn refuses_a_row_outside_the_matrix_without_sending() {
        let mut keychron = keychron(&[v5_max_opening()]);
        assert!(matches!(
            keychron.mixed_rgb().unwrap().leds_in_row(V5_MAX, 6),
            Err(KeychronError::Invalid(_))
        ));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn rejects_a_row_lookup_naming_an_led_past_the_led_count() {
        let mut leds = [0xffu8; 19];
        leds[18] = 98;
        let mut keychron = keychron(&[
            v5_max_opening(),
            exchange(
                &[0xa8, 0x06, 0, 0xff, 0xff, 0x07],
                &reply(&[0xa8, 0x06, 0x00], &leds),
            ),
        ]);
        assert!(matches!(
            keychron.mixed_rgb().unwrap().leds_in_row(V5_MAX, 0),
            Err(KeychronError::Invalid(_))
        ));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn saves_all_keychron_rgb_settings() {
        let mut keychron = keychron(&[
            v5_max_opening(),
            exchange(&[0xa8, 0x02], &[0xa8, 0x02, 0x00]),
        ]);
        assert_eq!(keychron.mixed_rgb().unwrap().save(), Ok(()));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }
}
