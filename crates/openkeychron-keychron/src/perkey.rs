//! Per-key RGB colours (`A8 01`, `02`, `05`, `06`, `09`, `0A`), from
//! `keyboards/keychron/common/rgb/keychron_rgb.c` and `per_key_rgb.c` at Keychron/qmk_firmware
//! `2b493ff9`. See `docs/protocol/keychron.md`, "Keychron RGB group" and "Per-key RGB".
//!
//! A colour set changes RAM only. [`PerKeyRgb::save`] writes it to EEPROM together with every
//! other Keychron RGB setting. The colours show only while the lighting effect is Keychron's
//! per-key effect, and every per-key effect takes its brightness from the global RGB brightness,
//! not from a key's stored value.
//!
//! Hardware verification is pending: the layouts come from source only (#20).

use openkeychron_hid::HidTransport;

use crate::client::{Keychron, KeychronError, RGB_GROUP};

const GET_PROTOCOL_VERSION: u8 = 0x01;
pub(crate) const SAVE: u8 = 0x02;
const GET_LED_COUNT: u8 = 0x05;
const GET_LED_INDEX: u8 = 0x06;
const GET_COLORS: u8 = 0x09;
const SET_COLORS: u8 = 0x0A;

/// The only Keychron RGB protocol version read in source (`PER_KEY_RGB_VER`).
const KNOWN_PROTOCOL_VERSION: u16 = 0x0001;

/// The firmware's limit on colours per get or set request.
const COLORS_PER_REQUEST: u8 = 9;

/// `A8 06` takes a 3-byte column mask.
const LED_INDEX_COLUMNS: u8 = 24;

/// `A8 06`'s answer for a column without an LED.
const NO_LED: u8 = 0xFF;

const FEATURE: &str = "per-key RGB (Keychron firmware 1.1.0 or later)";

/// The keyboard's matrix size, from its layout definition. `A8 06` only fills the reply for the
/// firmware's own columns, so a position outside the matrix would read a stale byte as an LED.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatrixSize {
    pub rows: u8,
    pub columns: u8,
}

/// One LED's stored colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PerKeyColor {
    pub hue: u8,
    pub saturation: u8,
    /// Stored, but ignored by every per-key effect: they use the global RGB brightness.
    pub value: u8,
}

/// Per-key RGB on a firmware that was checked to support it. Holds the LED count, so every
/// request stays inside the firmware's arrays: its handlers do not check `start + count`.
#[derive(Debug)]
pub struct PerKeyRgb<'keychron, T> {
    keychron: &'keychron mut Keychron<T>,
    led_count: u8,
}

impl<T: HidTransport> Keychron<T> {
    /// Checks the Keychron RGB feature bit (`0xA2`) and protocol version (`A8 01`), then reads
    /// the LED count (`A8 05`). Fails with [`KeychronError::Unsupported`] on firmware without
    /// per-key RGB, before anything is changed.
    pub fn per_key_rgb(&mut self) -> Result<PerKeyRgb<'_, T>, KeychronError> {
        let led_count = self.keychron_rgb_led_count(FEATURE)?;
        Ok(PerKeyRgb {
            keychron: self,
            led_count,
        })
    }

    /// The checks every Keychron RGB session starts with: the feature bit (`0xA2`) and protocol
    /// version (`A8 01`), then the LED count (`A8 05`). `feature` names what is unsupported.
    pub(crate) fn keychron_rgb_led_count(
        &mut self,
        feature: &'static str,
    ) -> Result<u8, KeychronError> {
        let features = match self.features() {
            Ok(features) => features,
            Err(KeychronError::Unhandled(_)) => return Err(KeychronError::Unsupported(feature)),
            Err(other) => return Err(other),
        };
        if !features.keychron_rgb() {
            return Err(KeychronError::Unsupported(feature));
        }
        let reply = self.group(RGB_GROUP, GET_PROTOCOL_VERSION, &[])?;
        let version = u16::from_le_bytes([reply[3], reply[4]]);
        if version != KNOWN_PROTOCOL_VERSION {
            return Err(KeychronError::UnknownRgbProtocol(version));
        }
        Ok(self.group(RGB_GROUP, GET_LED_COUNT, &[])?[3])
    }

    /// `A8 06` for the columns of `row` set in `mask`: the first LED under each column of
    /// `matrix`, `None` for a column without one or outside `mask`. Refuses a row outside
    /// `matrix` without sending: the firmware's row check is off by one (`row > MATRIX_ROWS`).
    pub(crate) fn leds_under(
        &mut self,
        led_count: u8,
        matrix: MatrixSize,
        row: u8,
        mask: u32,
    ) -> Result<Vec<Option<u8>>, KeychronError> {
        if matrix.columns > LED_INDEX_COLUMNS {
            return Err(KeychronError::Invalid(format!(
                "a matrix of {} columns does not fit the {LED_INDEX_COLUMNS}-column LED lookup",
                matrix.columns
            )));
        }
        if row >= matrix.rows {
            return Err(outside(matrix, row, 0));
        }
        let [mask_low, mask_middle, mask_high, _] = mask.to_le_bytes();
        let reply = self.group(
            RGB_GROUP,
            GET_LED_INDEX,
            &[row, mask_low, mask_middle, mask_high],
        )?;
        (0..matrix.columns)
            .map(|column| {
                if mask & (1 << column) == 0 {
                    return Ok(None);
                }
                match reply[3 + usize::from(column)] {
                    NO_LED => Ok(None),
                    led if led < led_count => Ok(Some(led)),
                    led => Err(KeychronError::Invalid(format!(
                        "the keyboard puts matrix position {row},{column} on LED {led}, past its \
                         {led_count} LEDs"
                    ))),
                }
            })
            .collect()
    }
}

fn outside(matrix: MatrixSize, row: u8, column: u8) -> KeychronError {
    KeychronError::Invalid(format!(
        "matrix position {row},{column} is outside the {}x{} matrix",
        matrix.rows, matrix.columns
    ))
}

impl<T: HidTransport> PerKeyRgb<'_, T> {
    #[must_use]
    pub const fn led_count(&self) -> u8 {
        self.led_count
    }

    /// `A8 06`: the first LED under a matrix position, `None` when it has none. Refuses a position
    /// outside `matrix` without sending: the firmware's row check is off by one
    /// (`row > MATRIX_ROWS`) and it never checks columns.
    pub fn led_index(
        &mut self,
        matrix: MatrixSize,
        row: u8,
        column: u8,
    ) -> Result<Option<u8>, KeychronError> {
        if column >= matrix.columns {
            return Err(outside(matrix, row, column));
        }
        // Past the mask width, `leds_under` refuses the matrix before the mask is used.
        let mask = 1u32.checked_shl(u32::from(column)).unwrap_or(0);
        let leds = self
            .keychron
            .leds_under(self.led_count, matrix, row, mask)?;
        Ok(leds[usize::from(column)])
    }

    /// `A8 09` for every LED, in requests of at most nine.
    pub fn colors(&mut self) -> Result<Vec<PerKeyColor>, KeychronError> {
        let mut colors = Vec::with_capacity(usize::from(self.led_count));
        let mut start = 0;
        while start < self.led_count {
            let batch = (self.led_count - start).min(COLORS_PER_REQUEST);
            colors.extend(self.read_colors(start, batch)?);
            start += batch;
        }
        Ok(colors)
    }

    /// `A8 09` for one LED.
    pub fn color(&mut self, led: u8) -> Result<PerKeyColor, KeychronError> {
        self.require_led(led)?;
        Ok(self.read_colors(led, 1)?[0])
    }

    /// Writes: `A8 0A` for one LED, then reads it back. Changes RAM only: call
    /// [`PerKeyRgb::save`] to keep it after a power cycle.
    pub fn set_color(&mut self, led: u8, color: PerKeyColor) -> Result<(), KeychronError> {
        self.require_led(led)?;
        self.keychron.group(
            RGB_GROUP,
            SET_COLORS,
            &[led, 1, color.hue, color.saturation, color.value],
        )?;
        let read = self.read_colors(led, 1)?[0];
        if read != color {
            return Err(KeychronError::ReadBackMismatch {
                written: format!("LED {led}: {color:?}"),
                read: format!("LED {led}: {read:?}"),
            });
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

    /// `A8 09` for `count` (at most nine) LEDs from `start`, already checked against the count.
    fn read_colors(&mut self, start: u8, count: u8) -> Result<Vec<PerKeyColor>, KeychronError> {
        let reply = self
            .keychron
            .group(RGB_GROUP, GET_COLORS, &[start, count])?;
        let (entries, _) = reply[3..].as_chunks::<3>();
        Ok(entries
            .iter()
            .take(usize::from(count))
            .map(|&[hue, saturation, value]| PerKeyColor {
                hue,
                saturation,
                value,
            })
            .collect())
    }

    fn require_led(&self, led: u8) -> Result<(), KeychronError> {
        if led < self.led_count {
            Ok(())
        } else {
            Err(KeychronError::Invalid(format!(
                "LED {led} does not exist; this keyboard has {}",
                self.led_count
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use openkeychron_hid::{DeviceIdentity, Event, Fixture, ReplayTransport};

    use super::{MatrixSize, PerKeyColor};
    use crate::client::{Keychron, KeychronError, REPORT_LEN};

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

    /// The support checks and LED count that open every per-key session.
    fn opening(led_count: u8) -> Vec<Event> {
        [
            exchange(&[0xa2], &FEATURES),
            exchange(&[0xa8, 0x01], &[0xa8, 0x01, 0x00, 0x01, 0x00]),
            exchange(&[0xa8, 0x05], &[0xa8, 0x05, 0x00, led_count]),
        ]
        .concat()
    }

    fn keychron(sessions: &[Vec<Event>]) -> Keychron<ReplayTransport> {
        let device = DeviceIdentity {
            name: Some("synthetic".to_owned()),
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

    const V5_MAX: MatrixSize = MatrixSize {
        rows: 6,
        columns: 19,
    };

    const RED: PerKeyColor = PerKeyColor {
        hue: 0,
        saturation: 255,
        value: 255,
    };

    #[test]
    fn reads_the_led_count_after_checking_support() {
        let mut keychron = keychron(&[opening(98)]);
        assert_eq!(keychron.per_key_rgb().map(|rgb| rgb.led_count()), Ok(98));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn says_unsupported_when_the_keychron_rgb_feature_bit_is_clear() {
        let mut keychron = keychron(&[exchange(&[0xa2], &[0xa2, 0x00, 0b0111_0111, 0x03])]);
        assert_eq!(
            keychron.per_key_rgb().map(|rgb| rgb.led_count()),
            Err(KeychronError::Unsupported(
                "per-key RGB (Keychron firmware 1.1.0 or later)"
            ))
        );
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn says_unsupported_when_the_firmware_has_no_feature_command() {
        let mut keychron = keychron(&[exchange(&[0xa2], &[0xff])]);
        assert_eq!(
            keychron.per_key_rgb().map(|rgb| rgb.led_count()),
            Err(KeychronError::Unsupported(
                "per-key RGB (Keychron firmware 1.1.0 or later)"
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
            keychron.per_key_rgb().map(|rgb| rgb.led_count()),
            Err(KeychronError::UnknownRgbProtocol(2))
        );
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn reads_the_led_under_a_matrix_position() {
        let mut reply = vec![0xa8, 0x06, 0x00];
        reply.extend([0xff; 19]);
        reply[3 + 1] = 38;
        let mut keychron = keychron(&[opening(98), exchange(&[0xa8, 0x06, 2, 0b10, 0, 0], &reply)]);
        assert_eq!(
            keychron.per_key_rgb().unwrap().led_index(V5_MAX, 2, 1),
            Ok(Some(38))
        );
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn reports_no_led_for_a_position_without_one() {
        let mut reply = vec![0xa8, 0x06, 0x00];
        reply.extend([0xff; 19]);
        let mut keychron = keychron(&[opening(98), exchange(&[0xa8, 0x06, 5, 0, 0x01, 0], &reply)]);
        assert_eq!(
            keychron.per_key_rgb().unwrap().led_index(V5_MAX, 5, 8),
            Ok(None)
        );
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn reads_column_twenty_three_from_the_last_mask_byte() {
        let mut reply = vec![0xa8, 0x06, 0x00];
        reply.extend([0xff; 24]);
        reply[3 + 23] = 7;
        let mut keychron = keychron(&[opening(98), exchange(&[0xa8, 0x06, 0, 0, 0, 0x80], &reply)]);
        let matrix = MatrixSize {
            rows: 1,
            columns: 24,
        };
        assert_eq!(
            keychron.per_key_rgb().unwrap().led_index(matrix, 0, 23),
            Ok(Some(7))
        );
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn refuses_a_row_outside_the_matrix_without_sending() {
        let mut keychron = keychron(&[opening(98)]);
        assert!(matches!(
            keychron.per_key_rgb().unwrap().led_index(V5_MAX, 6, 0),
            Err(KeychronError::Invalid(_))
        ));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn refuses_a_column_outside_the_matrix_without_sending() {
        let mut keychron = keychron(&[opening(98)]);
        assert!(matches!(
            keychron.per_key_rgb().unwrap().led_index(V5_MAX, 0, 19),
            Err(KeychronError::Invalid(_))
        ));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn refuses_a_matrix_wider_than_the_request_mask_without_sending() {
        let mut keychron = keychron(&[opening(98)]);
        let matrix = MatrixSize {
            rows: 6,
            columns: 25,
        };
        assert!(matches!(
            keychron.per_key_rgb().unwrap().led_index(matrix, 0, 24),
            Err(KeychronError::Invalid(_))
        ));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn refuses_a_column_past_the_request_mask_width_without_sending() {
        let mut keychron = keychron(&[opening(98)]);
        let matrix = MatrixSize {
            rows: 6,
            columns: 40,
        };
        assert!(matches!(
            keychron.per_key_rgb().unwrap().led_index(matrix, 0, 35),
            Err(KeychronError::Invalid(_))
        ));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn rejects_an_led_index_past_the_led_count() {
        let mut reply = vec![0xa8, 0x06, 0x00, 98];
        reply.extend([0xff; 18]);
        let mut keychron = keychron(&[opening(98), exchange(&[0xa8, 0x06, 0, 0b1, 0, 0], &reply)]);
        assert!(matches!(
            keychron.per_key_rgb().unwrap().led_index(V5_MAX, 0, 0),
            Err(KeychronError::Invalid(_))
        ));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn reads_all_colours_in_batches_of_nine() {
        let mut first = vec![0xa8, 0x09, 0x00];
        first.extend([170, 255, 255]);
        first.extend([0; 24]);
        let mut second = vec![0xa8, 0x09, 0x00];
        second.extend([0, 0, 0, 85, 128, 64]);
        let mut keychron = keychron(&[
            opening(11),
            exchange(&[0xa8, 0x09, 0, 9], &first),
            exchange(&[0xa8, 0x09, 9, 2], &second),
        ]);
        let colors = keychron.per_key_rgb().unwrap().colors().unwrap();
        assert_eq!(colors.len(), 11);
        assert_eq!(
            colors[0],
            PerKeyColor {
                hue: 170,
                saturation: 255,
                value: 255,
            }
        );
        assert_eq!(
            colors[10],
            PerKeyColor {
                hue: 85,
                saturation: 128,
                value: 64,
            }
        );
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn sets_a_colour_and_reads_it_back() {
        let mut keychron = keychron(&[
            opening(98),
            exchange(
                &[0xa8, 0x0a, 38, 1, 0, 255, 255],
                &[0xa8, 0x0a, 0x00, 1, 0, 255, 255],
            ),
            exchange(&[0xa8, 0x09, 38, 1], &[0xa8, 0x09, 0x00, 0, 255, 255]),
        ]);
        assert_eq!(keychron.per_key_rgb().unwrap().set_color(38, RED), Ok(()));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn fails_when_a_colour_reads_back_differently() {
        let mut keychron = keychron(&[
            opening(98),
            exchange(
                &[0xa8, 0x0a, 38, 1, 0, 255, 255],
                &[0xa8, 0x0a, 0x00, 1, 0, 255, 255],
            ),
            exchange(&[0xa8, 0x09, 38, 1], &[0xa8, 0x09, 0x00, 170, 255, 255]),
        ]);
        assert!(matches!(
            keychron.per_key_rgb().unwrap().set_color(38, RED),
            Err(KeychronError::ReadBackMismatch { .. })
        ));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn refuses_an_led_past_the_led_count_without_sending() {
        let mut keychron = keychron(&[opening(98)]);
        assert!(matches!(
            keychron.per_key_rgb().unwrap().set_color(98, RED),
            Err(KeychronError::Invalid(_))
        ));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn reports_a_refused_colour_set() {
        let mut keychron = keychron(&[
            opening(98),
            exchange(&[0xa8, 0x0a, 38, 1, 0, 255, 255], &[0xa8, 0x0a, 0x01]),
        ]);
        assert_eq!(
            keychron.per_key_rgb().unwrap().set_color(38, RED),
            Err(KeychronError::Refused("0xa8 0x0a".to_owned()))
        );
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn saves_all_keychron_rgb_settings() {
        let mut keychron = keychron(&[opening(98), exchange(&[0xa8, 0x02], &[0xa8, 0x02, 0x00])]);
        assert_eq!(keychron.per_key_rgb().unwrap().save(), Ok(()));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }
}
