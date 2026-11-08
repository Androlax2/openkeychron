//! What `read_settings` returns: debounce, Snap Click and wireless timers. Only reads.

use openkeychron_hid::HidTransport;
use openkeychron_keychron::{self as keychron, Keychron};
use serde::Serialize;

use crate::error::CommandError;

#[derive(Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub debounce: Debounce,
    /// Every pair the firmware holds, `off` ones included, in firmware order from pair 0.
    pub snap_click_pairs: Vec<SnapClickPair>,
    pub wireless: WirelessTimers,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Debounce {
    pub algorithm: DebounceAlgorithm,
    pub time_ms: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum DebounceAlgorithm {
    SymmetricDeferGlobal,
    SymmetricDeferPerRow,
    SymmetricDeferPerKey,
    SymmetricEagerPerRow,
    SymmetricEagerPerKey,
    AsymmetricEagerDeferPerKey,
    None,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SnapClickPair {
    pub mode: SnapClickMode,
    /// Two 8-bit basic keycodes.
    pub keys: [u8; 2],
}

/// Keychron's "LKP" is most likely `last-input`: an inference, see docs/protocol/keychron.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum SnapClickMode {
    Off,
    Regular,
    LastInput,
    FirstKey,
    SecondKey,
    Neutral,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct WirelessTimers {
    pub backlight_off_seconds: u16,
    pub sleep_seconds: u16,
}

pub fn read<T: HidTransport>(transport: T) -> Result<Settings, CommandError> {
    let mut keychron = Keychron::new(transport);
    let debounce = keychron.debounce()?;
    let pairs = keychron.snap_click_pairs()?;
    let wireless = keychron.wireless_timers()?;
    Ok(Settings {
        debounce: Debounce {
            algorithm: debounce.algorithm.into(),
            time_ms: debounce.time_ms,
        },
        snap_click_pairs: pairs
            .into_iter()
            .map(|pair| SnapClickPair {
                mode: pair.mode.into(),
                keys: pair.keys,
            })
            .collect(),
        wireless: WirelessTimers {
            backlight_off_seconds: wireless.backlight_off_seconds,
            sleep_seconds: wireless.sleep_seconds,
        },
    })
}

impl From<keychron::DebounceAlgorithm> for DebounceAlgorithm {
    fn from(algorithm: keychron::DebounceAlgorithm) -> Self {
        match algorithm {
            keychron::DebounceAlgorithm::SymmetricDeferGlobal => Self::SymmetricDeferGlobal,
            keychron::DebounceAlgorithm::SymmetricDeferPerRow => Self::SymmetricDeferPerRow,
            keychron::DebounceAlgorithm::SymmetricDeferPerKey => Self::SymmetricDeferPerKey,
            keychron::DebounceAlgorithm::SymmetricEagerPerRow => Self::SymmetricEagerPerRow,
            keychron::DebounceAlgorithm::SymmetricEagerPerKey => Self::SymmetricEagerPerKey,
            keychron::DebounceAlgorithm::AsymmetricEagerDeferPerKey => {
                Self::AsymmetricEagerDeferPerKey
            }
            keychron::DebounceAlgorithm::None => Self::None,
        }
    }
}

impl From<keychron::SnapClickMode> for SnapClickMode {
    fn from(mode: keychron::SnapClickMode) -> Self {
        match mode {
            keychron::SnapClickMode::Off => Self::Off,
            keychron::SnapClickMode::Regular => Self::Regular,
            keychron::SnapClickMode::LastInput => Self::LastInput,
            keychron::SnapClickMode::FirstKey => Self::FirstKey,
            keychron::SnapClickMode::SecondKey => Self::SecondKey,
            keychron::SnapClickMode::Neutral => Self::Neutral,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Debounce, DebounceAlgorithm, SnapClickMode, SnapClickPair, WirelessTimers, read};
    use crate::test_support::{lend, replay};

    #[test]
    fn reads_debounce_snap_click_and_wireless_timers() {
        let mut replay = replay("cli-settings-get.toml");
        let settings = read(lend(&mut replay.transport)).unwrap();
        assert_eq!(
            settings.debounce,
            Debounce {
                algorithm: DebounceAlgorithm::SymmetricDeferGlobal,
                time_ms: 50,
            }
        );
        assert_eq!(
            settings.snap_click_pairs[0],
            SnapClickPair {
                mode: SnapClickMode::LastInput,
                keys: [0x04, 0x07],
            }
        );
        assert!(
            settings.snap_click_pairs[1..]
                .iter()
                .all(|pair| pair.mode == SnapClickMode::Off),
            "{:?}",
            settings.snap_click_pairs
        );
        assert_eq!(
            settings.wireless,
            WirelessTimers {
                backlight_off_seconds: 600,
                sleep_seconds: 7200,
            }
        );
        assert_eq!(replay.finish(), vec![]);
    }
}
