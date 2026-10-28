//! Wireless sleep timers (`A7 0B`, `A7 0C`), as documented in `docs/protocol/keychron.md`.
//!
//! The set command writes EEPROM at once and re-initialises the wireless module. While the
//! keyboard is connected wirelessly it also sends a small mouse movement to the computer, which
//! is one more reason the CLI only writes over a cable.

use openkeychron_hid::HidTransport;

use crate::client::{Keychron, KeychronError};

const WIRELESS_LPM_GET: u8 = 0x0B;
const WIRELESS_LPM_SET: u8 = 0x0C;

/// The firmware refuses shorter values (`common/wireless/wireless.c`, `wireless_lpm_set`).
pub const MIN_BACKLIGHT_OFF_SECONDS: u16 = 5;
pub const MIN_SLEEP_SECONDS: u16 = 60;

/// When the backlight turns off and the keyboard sleeps, in seconds of inactivity, while
/// connected wirelessly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WirelessTimers {
    pub backlight_off_seconds: u16,
    pub sleep_seconds: u16,
}

impl<T: HidTransport> Keychron<T> {
    /// `A7 0B`.
    pub fn wireless_timers(&mut self) -> Result<WirelessTimers, KeychronError> {
        self.require_wireless_timers()?;
        let reply = self.misc(WIRELESS_LPM_GET, &[])?;
        Ok(WirelessTimers {
            backlight_off_seconds: u16::from_le_bytes([reply[3], reply[4]]),
            sleep_seconds: u16::from_le_bytes([reply[5], reply[6]]),
        })
    }

    /// `A7 0C`, then reads the values back. The firmware writes them to EEPROM at once.
    pub fn set_wireless_timers(&mut self, timers: WirelessTimers) -> Result<(), KeychronError> {
        if timers.backlight_off_seconds < MIN_BACKLIGHT_OFF_SECONDS {
            return Err(KeychronError::Invalid(format!(
                "the backlight-off time must be at least {MIN_BACKLIGHT_OFF_SECONDS} s"
            )));
        }
        if timers.sleep_seconds < MIN_SLEEP_SECONDS {
            return Err(KeychronError::Invalid(format!(
                "the sleep time must be at least {MIN_SLEEP_SECONDS} s"
            )));
        }
        self.require_wireless_timers()?;
        let [backlight_low, backlight_high] = timers.backlight_off_seconds.to_le_bytes();
        let [sleep_low, sleep_high] = timers.sleep_seconds.to_le_bytes();
        self.misc(
            WIRELESS_LPM_SET,
            &[backlight_low, backlight_high, sleep_low, sleep_high],
        )?;
        let read = self.wireless_timers()?;
        if read != timers {
            return Err(KeychronError::ReadBackMismatch {
                written: format!("{timers:?}"),
                read: format!("{read:?}"),
            });
        }
        Ok(())
    }

    fn require_wireless_timers(&mut self) -> Result<(), KeychronError> {
        if self.misc_features()?.wireless_timers() {
            Ok(())
        } else {
            Err(KeychronError::Unsupported("wireless sleep timers"))
        }
    }
}

#[cfg(test)]
mod tests {
    use openkeychron_hid::{DeviceIdentity, Event, Fixture, ReplayTransport};

    use super::WirelessTimers;
    use crate::client::{Keychron, KeychronError, REPORT_LEN};

    /// `A7 01` reply advertising wireless sleep timers (bit 4).
    const MISC_FEATURES: [u8; 6] = [0xa7, 0x01, 0x00, 0x02, 0x00, 0b0001_0000];

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
    fn reads_wireless_timers_as_little_endian_seconds() {
        // 600 s = 0x0258, 7200 s = 0x1c20: the firmware defaults.
        let mut keychron = keychron(&[
            features(),
            exchange(&[0xa7, 0x0b], &[0xa7, 0x0b, 0x00, 0x58, 0x02, 0x20, 0x1c]),
        ]);
        assert_eq!(
            keychron.wireless_timers(),
            Ok(WirelessTimers {
                backlight_off_seconds: 600,
                sleep_seconds: 7200,
            })
        );
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn sets_wireless_timers_and_reads_them_back() {
        let mut keychron = keychron(&[
            features(),
            exchange(&[0xa7, 0x0c, 0x1e, 0x00, 0x10, 0x0e], &[0xa7, 0x0c, 0x00]),
            features(),
            exchange(&[0xa7, 0x0b], &[0xa7, 0x0b, 0x00, 0x1e, 0x00, 0x10, 0x0e]),
        ]);
        let wanted = WirelessTimers {
            backlight_off_seconds: 30,
            sleep_seconds: 3600,
        };
        assert_eq!(keychron.set_wireless_timers(wanted), Ok(()));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn fails_when_wireless_timers_read_back_differently() {
        let mut keychron = keychron(&[
            features(),
            exchange(&[0xa7, 0x0c, 0x1e, 0x00, 0x10, 0x0e], &[0xa7, 0x0c, 0x00]),
            features(),
            exchange(&[0xa7, 0x0b], &[0xa7, 0x0b, 0x00, 0x58, 0x02, 0x20, 0x1c]),
        ]);
        let result = keychron.set_wireless_timers(WirelessTimers {
            backlight_off_seconds: 30,
            sleep_seconds: 3600,
        });
        assert!(matches!(
            result,
            Err(KeychronError::ReadBackMismatch { .. })
        ));
    }

    #[test]
    fn refuses_a_backlight_time_under_5_seconds_without_sending() {
        let mut keychron = keychron(&[]);
        let result = keychron.set_wireless_timers(WirelessTimers {
            backlight_off_seconds: 4,
            sleep_seconds: 3600,
        });
        assert!(matches!(result, Err(KeychronError::Invalid(_))));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn refuses_a_sleep_time_under_60_seconds_without_sending() {
        let mut keychron = keychron(&[]);
        let result = keychron.set_wireless_timers(WirelessTimers {
            backlight_off_seconds: 30,
            sleep_seconds: 59,
        });
        assert!(matches!(result, Err(KeychronError::Invalid(_))));
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn refuses_wireless_timers_when_the_firmware_does_not_report_them() {
        let mut keychron = keychron(&[exchange(
            &[0xa7, 0x01],
            &[0xa7, 0x01, 0x00, 0x02, 0x00, 0b0000_1100],
        )]);
        assert_eq!(
            keychron.wireless_timers(),
            Err(KeychronError::Unsupported("wireless sleep timers"))
        );
    }
}
