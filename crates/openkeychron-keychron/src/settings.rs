//! Debounce and Snap Click (`A7 05`–`A7 0A`), as documented in `docs/protocol/keychron.md`.
//!
//! Debounce is written to EEPROM by the set command itself. Snap Click pairs change in RAM and
//! reach EEPROM only through [`Keychron::save_snap_click`].

use openkeychron_hid::HidTransport;

use crate::client::{Keychron, KeychronError};

const DEBOUNCE_GET: u8 = 0x05;
const DEBOUNCE_SET: u8 = 0x06;
const SNAP_CLICK_GET_INFO: u8 = 0x07;
const SNAP_CLICK_GET: u8 = 0x08;
const SNAP_CLICK_SET: u8 = 0x09;
const SNAP_CLICK_SAVE: u8 = 0x0A;

/// The firmware's limit on pairs per get or set request.
const SNAP_CLICK_PAIRS_PER_REQUEST: u8 = 9;

/// The asymmetric algorithm caps its time at 127 ms; refusing larger values keeps the read-back
/// equal to what was asked.
const ASYMMETRIC_MAX_MS: u8 = 127;

/// Debounce algorithms, in the firmware's order (`common/debounce/keychron_debounce.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebounceAlgorithm {
    SymmetricDeferGlobal,
    SymmetricDeferPerRow,
    SymmetricDeferPerKey,
    SymmetricEagerPerRow,
    SymmetricEagerPerKey,
    AsymmetricEagerDeferPerKey,
    None,
}

impl DebounceAlgorithm {
    const ALL: [Self; 7] = [
        Self::SymmetricDeferGlobal,
        Self::SymmetricDeferPerRow,
        Self::SymmetricDeferPerKey,
        Self::SymmetricEagerPerRow,
        Self::SymmetricEagerPerKey,
        Self::AsymmetricEagerDeferPerKey,
        Self::None,
    ];

    #[must_use]
    pub const fn id(self) -> u8 {
        self as u8
    }

    pub fn from_id(id: u8) -> Result<Self, KeychronError> {
        Self::ALL
            .get(usize::from(id))
            .copied()
            .ok_or_else(|| KeychronError::Invalid(format!("unknown debounce algorithm {id}")))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Debounce {
    pub algorithm: DebounceAlgorithm,
    pub time_ms: u8,
}

/// Snap Click modes (`common/snap_click/snap_click.c`). Keychron's "LKP" is most likely
/// [`SnapClickMode::LastInput`]; that mapping is an inference, not documented by Keychron.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapClickMode {
    Off,
    Regular,
    LastInput,
    FirstKey,
    SecondKey,
    Neutral,
}

impl SnapClickMode {
    const ALL: [Self; 6] = [
        Self::Off,
        Self::Regular,
        Self::LastInput,
        Self::FirstKey,
        Self::SecondKey,
        Self::Neutral,
    ];

    #[must_use]
    pub const fn id(self) -> u8 {
        self as u8
    }

    pub fn from_id(id: u8) -> Result<Self, KeychronError> {
        Self::ALL
            .get(usize::from(id))
            .copied()
            .ok_or_else(|| KeychronError::Invalid(format!("unknown Snap Click mode {id}")))
    }
}

/// Two keys that Snap Click arbitrates. Keys are 8-bit basic keycodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapClickPair {
    pub mode: SnapClickMode,
    pub keys: [u8; 2],
}

impl SnapClickPair {
    fn validate(self) -> Result<(), KeychronError> {
        if self.mode != SnapClickMode::Off && self.keys.contains(&0) {
            return Err(KeychronError::Invalid(
                "an active Snap Click pair needs two non-zero keycodes".to_owned(),
            ));
        }
        Ok(())
    }
}

impl<T: HidTransport> Keychron<T> {
    /// `A7 05`.
    pub fn debounce(&mut self) -> Result<Debounce, KeychronError> {
        self.require_debounce()?;
        let reply = self.misc(DEBOUNCE_GET, &[])?;
        Ok(Debounce {
            algorithm: DebounceAlgorithm::from_id(reply[4])?,
            time_ms: reply[5],
        })
    }

    /// `A7 06`, then reads the value back. The firmware writes it to EEPROM at once.
    pub fn set_debounce(&mut self, debounce: Debounce) -> Result<(), KeychronError> {
        if debounce.algorithm == DebounceAlgorithm::AsymmetricEagerDeferPerKey
            && debounce.time_ms > ASYMMETRIC_MAX_MS
        {
            return Err(KeychronError::Invalid(format!(
                "the asymmetric algorithm allows at most {ASYMMETRIC_MAX_MS} ms"
            )));
        }
        self.require_debounce()?;
        self.misc(DEBOUNCE_SET, &[debounce.algorithm.id(), debounce.time_ms])?;
        let read = self.debounce()?;
        if read != debounce {
            return Err(KeychronError::ReadBackMismatch {
                written: format!("{debounce:?}"),
                read: format!("{read:?}"),
            });
        }
        Ok(())
    }

    /// `A7 07`: how many Snap Click pairs the firmware holds (20 on the V5 Max).
    pub fn snap_click_count(&mut self) -> Result<u8, KeychronError> {
        self.require_snap_click()?;
        Ok(self.misc(SNAP_CLICK_GET_INFO, &[])?[3])
    }

    /// `A7 08`, in requests of at most nine pairs.
    pub fn snap_click_pairs(&mut self) -> Result<Vec<SnapClickPair>, KeychronError> {
        let count = self.snap_click_count()?;
        let mut pairs = Vec::with_capacity(usize::from(count));
        let mut start = 0;
        while start < count {
            let batch = (count - start).min(SNAP_CLICK_PAIRS_PER_REQUEST);
            let reply = self.misc(SNAP_CLICK_GET, &[start, batch])?;
            let (entries, _) = reply[3..].as_chunks::<3>();
            for [mode, first, second] in entries.iter().take(usize::from(batch)) {
                pairs.push(SnapClickPair {
                    mode: SnapClickMode::from_id(*mode)?,
                    keys: [*first, *second],
                });
            }
            start += batch;
        }
        Ok(pairs)
    }

    /// `A7 09` for one pair, then reads it back. Changes RAM only: call
    /// [`Keychron::save_snap_click`] to keep it after a power cycle.
    pub fn set_snap_click_pair(
        &mut self,
        index: u8,
        pair: SnapClickPair,
    ) -> Result<(), KeychronError> {
        pair.validate()?;
        let count = self.snap_click_count()?;
        if index >= count {
            return Err(KeychronError::Invalid(format!(
                "Snap Click pair {index} does not exist; this firmware has {count}"
            )));
        }
        self.misc(
            SNAP_CLICK_SET,
            &[index, 1, pair.mode.id(), pair.keys[0], pair.keys[1]],
        )?;
        let read = self.misc(SNAP_CLICK_GET, &[index, 1])?;
        let read = SnapClickPair {
            mode: SnapClickMode::from_id(read[3])?,
            keys: [read[4], read[5]],
        };
        if read != pair {
            return Err(KeychronError::ReadBackMismatch {
                written: format!("{pair:?}"),
                read: format!("{read:?}"),
            });
        }
        Ok(())
    }

    /// `A7 0A`: writes all Snap Click pairs to EEPROM.
    pub fn save_snap_click(&mut self) -> Result<(), KeychronError> {
        self.require_snap_click()?;
        self.misc(SNAP_CLICK_SAVE, &[])?;
        Ok(())
    }

    fn require_debounce(&mut self) -> Result<(), KeychronError> {
        if self.misc_features()?.debounce() {
            Ok(())
        } else {
            Err(KeychronError::Unsupported("dynamic debounce"))
        }
    }

    fn require_snap_click(&mut self) -> Result<(), KeychronError> {
        if self.misc_features()?.snap_click() {
            Ok(())
        } else {
            Err(KeychronError::Unsupported("Snap Click"))
        }
    }
}

#[cfg(test)]
mod tests {
    use openkeychron_hid::{DeviceIdentity, Event, Fixture, ReplayTransport};

    use super::{Debounce, DebounceAlgorithm, SnapClickMode, SnapClickPair};
    use crate::client::{Keychron, KeychronError, REPORT_LEN};

    /// `A7 01` reply advertising debounce (bit 2) and Snap Click (bit 3).
    const MISC_FEATURES: [u8; 6] = [0xa7, 0x01, 0x00, 0x02, 0x00, 0b0000_1100];

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

    fn features() -> Vec<Event> {
        exchange(&[0xa7, 0x01], &MISC_FEATURES)
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

    #[test]
    fn reads_debounce() {
        let mut keychron = keychron(&[
            features(),
            exchange(&[0xa7, 0x05], &[0xa7, 0x05, 0x00, 0x00, 0x00, 50]),
        ]);
        assert_eq!(
            keychron.debounce(),
            Ok(Debounce {
                algorithm: DebounceAlgorithm::SymmetricDeferGlobal,
                time_ms: 50,
            })
        );
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn sets_debounce_and_reads_it_back() {
        let mut keychron = keychron(&[
            features(),
            exchange(&[0xa7, 0x06, 0x04, 10], &[0xa7, 0x06, 0x00, 10]),
            features(),
            exchange(&[0xa7, 0x05], &[0xa7, 0x05, 0x00, 0x00, 0x04, 10]),
        ]);
        let wanted = Debounce {
            algorithm: DebounceAlgorithm::SymmetricEagerPerKey,
            time_ms: 10,
        };
        assert_eq!(keychron.set_debounce(wanted), Ok(()));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn fails_when_debounce_reads_back_differently() {
        let mut keychron = keychron(&[
            features(),
            exchange(&[0xa7, 0x06, 0x04, 10], &[0xa7, 0x06, 0x00, 10]),
            features(),
            exchange(&[0xa7, 0x05], &[0xa7, 0x05, 0x00, 0x00, 0x00, 50]),
        ]);
        let result = keychron.set_debounce(Debounce {
            algorithm: DebounceAlgorithm::SymmetricEagerPerKey,
            time_ms: 10,
        });
        assert!(matches!(
            result,
            Err(KeychronError::ReadBackMismatch { .. })
        ));
    }

    #[test]
    fn refuses_asymmetric_debounce_above_127_ms_without_sending() {
        let mut keychron = keychron(&[]);
        let result = keychron.set_debounce(Debounce {
            algorithm: DebounceAlgorithm::AsymmetricEagerDeferPerKey,
            time_ms: 128,
        });
        assert!(matches!(result, Err(KeychronError::Invalid(_))));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn refuses_debounce_when_the_firmware_does_not_report_it() {
        let mut keychron = keychron(&[exchange(
            &[0xa7, 0x01],
            &[0xa7, 0x01, 0x00, 0x02, 0x00, 0b0000_1000],
        )]);
        assert_eq!(
            keychron.debounce(),
            Err(KeychronError::Unsupported("dynamic debounce"))
        );
    }

    #[test]
    fn reports_a_refused_set() {
        let mut keychron = keychron(&[
            features(),
            exchange(&[0xa7, 0x06, 0x04, 10], &[0xa7, 0x06, 0x01]),
        ]);
        let result = keychron.set_debounce(Debounce {
            algorithm: DebounceAlgorithm::SymmetricEagerPerKey,
            time_ms: 10,
        });
        assert_eq!(result, Err(KeychronError::Refused("0xa7 0x06".to_owned())));
    }

    #[test]
    fn reads_snap_click_pairs_in_batches_of_nine() {
        let mut first = vec![0xa7, 0x08, 0x00];
        first.extend([2, 0x04, 0x07]);
        let sessions = vec![
            features(),
            exchange(&[0xa7, 0x07], &[0xa7, 0x07, 0x00, 11]),
            exchange(&[0xa7, 0x08, 0, 9], &first),
            exchange(&[0xa7, 0x08, 9, 2], &[0xa7, 0x08, 0x00]),
        ];
        let mut keychron = keychron(&sessions);
        let pairs = keychron.snap_click_pairs().unwrap();
        assert_eq!(pairs.len(), 11);
        assert_eq!(
            pairs[0],
            SnapClickPair {
                mode: SnapClickMode::LastInput,
                keys: [0x04, 0x07],
            }
        );
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn sets_a_snap_click_pair_and_reads_it_back() {
        let mut keychron = keychron(&[
            features(),
            exchange(&[0xa7, 0x07], &[0xa7, 0x07, 0x00, 20]),
            exchange(&[0xa7, 0x09, 3, 1, 2, 0x04, 0x07], &[0xa7, 0x09, 0x00]),
            exchange(&[0xa7, 0x08, 3, 1], &[0xa7, 0x08, 0x00, 2, 0x04, 0x07]),
        ]);
        let pair = SnapClickPair {
            mode: SnapClickMode::LastInput,
            keys: [0x04, 0x07],
        };
        assert_eq!(keychron.set_snap_click_pair(3, pair), Ok(()));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn refuses_a_snap_click_index_past_the_firmware_count() {
        let mut keychron =
            keychron(&[features(), exchange(&[0xa7, 0x07], &[0xa7, 0x07, 0x00, 20])]);
        let pair = SnapClickPair {
            mode: SnapClickMode::Off,
            keys: [0, 0],
        };
        assert!(matches!(
            keychron.set_snap_click_pair(20, pair),
            Err(KeychronError::Invalid(_))
        ));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn refuses_an_active_snap_click_pair_with_a_zero_key_without_sending() {
        let mut keychron = keychron(&[]);
        let pair = SnapClickPair {
            mode: SnapClickMode::Regular,
            keys: [0x04, 0],
        };
        assert!(matches!(
            keychron.set_snap_click_pair(0, pair),
            Err(KeychronError::Invalid(_))
        ));
    }

    #[test]
    fn saves_snap_click() {
        let mut keychron = keychron(&[features(), exchange(&[0xa7, 0x0a], &[0xa7, 0x0a, 0x00])]);
        assert_eq!(keychron.save_snap_click(), Ok(()));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }
}
