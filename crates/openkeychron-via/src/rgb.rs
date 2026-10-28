//! The RGB matrix through VIA's lighting channel (`id_qmk_rgb_matrix_channel`), as handled in
//! <https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/via.c>
//! (`via_qmk_rgb_matrix_get_value`, `_set_value`, `_save`). Firmware behaviour that shapes this
//! module:
//!
//! - A set only changes RAM (`*_noeeprom`); `id_custom_save` writes EEPROM.
//! - Save writes only when **exactly one** value was set since the previous save. So each change
//!   is saved on its own, right after its read-back. If another program changed lighting without
//!   saving since the keyboard started, the firmware silently skips this save too.
//! - Brightness is scaled on the way in: setting `v` stores `(v × 255) >> 8` (`scale8`, not the
//!   "fixed" variant, with `RGB_MATRIX_MAXIMUM_BRIGHTNESS` = 255 on the V5 Max), so 255 reads back
//!   as 254. The read-back expects exactly that.
//! - Effect 0 means lighting off. Other effect numbers depend on the animations the firmware was
//!   built with, and out-of-range numbers are clamped rather than refused; the read-back catches
//!   that, and nothing is saved on a mismatch.

use openkeychron_hid::HidTransport;

use crate::client::{Via, ViaError};
use crate::command::Command;

/// `id_qmk_rgb_matrix_channel`.
const RGB_MATRIX_CHANNEL: u8 = 3;

const BRIGHTNESS: u8 = 1;
const EFFECT: u8 = 2;
const EFFECT_SPEED: u8 = 3;
const COLOR: u8 = 4;

/// The RGB matrix as the keyboard reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RgbMatrix {
    pub brightness: u8,
    /// 0 when lighting is off.
    pub effect: u8,
    pub speed: u8,
    pub hue: u8,
    pub saturation: u8,
}

/// One lighting value to change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RgbValue {
    Brightness(u8),
    Effect(u8),
    Speed(u8),
    Color { hue: u8, saturation: u8 },
}

impl RgbValue {
    const fn id(self) -> u8 {
        match self {
            Self::Brightness(_) => BRIGHTNESS,
            Self::Effect(_) => EFFECT,
            Self::Speed(_) => EFFECT_SPEED,
            Self::Color { .. } => COLOR,
        }
    }

    fn data(self) -> Vec<u8> {
        match self {
            Self::Brightness(value) | Self::Effect(value) | Self::Speed(value) => vec![value],
            Self::Color { hue, saturation } => vec![hue, saturation],
        }
    }

    /// What the firmware reports after this value is set.
    #[must_use]
    pub fn expected_read_back(self) -> Self {
        match self {
            Self::Brightness(value) => Self::Brightness(stored_brightness(value)),
            other => other,
        }
    }
}

/// `scale8(value, 255)` without the "fixed" rounding: `(value × 255) >> 8`.
#[must_use]
pub fn stored_brightness(value: u8) -> u8 {
    let scaled = (u16::from(value) * 255) >> 8;
    u8::try_from(scaled).unwrap_or(u8::MAX)
}

/// The value to send so the keyboard stores `stored` again, or `None` when no value can
/// (255 cannot be reached through VIA: the highest it stores is 254).
#[must_use]
pub fn brightness_to_store(stored: u8) -> Option<u8> {
    (0..=u8::MAX).find(|&value| stored_brightness(value) == stored)
}

impl<T: HidTransport> Via<T> {
    /// Reads brightness, effect, speed and colour. Only reads.
    pub fn rgb_matrix(&mut self) -> Result<RgbMatrix, ViaError> {
        let brightness = self.lighting_value(BRIGHTNESS)?;
        let effect = self.lighting_value(EFFECT)?;
        let speed = self.lighting_value(EFFECT_SPEED)?;
        let color = self.lighting_value(COLOR)?;
        Ok(RgbMatrix {
            brightness: brightness[0],
            effect: effect[0],
            speed: speed[0],
            hue: color[0],
            saturation: color[1],
        })
    }

    /// Sets one value in RAM, reads it back, and saves it to EEPROM only if it matches.
    pub fn set_rgb_value(&mut self, value: RgbValue) -> Result<(), ViaError> {
        let mut arguments = vec![RGB_MATRIX_CHANNEL, value.id()];
        arguments.extend(value.data());
        let reply = self.exchange(Command::CustomSetValue, &arguments)?;
        check_lighting_echo(&reply, value.id())?;

        let read = self.read_value_like(value)?;
        let expected = value.expected_read_back();
        if read != expected {
            return Err(ViaError::ReadBackMismatch {
                written: format!("{value:?} (expected to read {expected:?})"),
                read: format!("{read:?}"),
            });
        }

        self.exchange(Command::CustomSave, &[RGB_MATRIX_CHANNEL])?;
        Ok(())
    }

    fn read_value_like(&mut self, value: RgbValue) -> Result<RgbValue, ViaError> {
        let data = self.lighting_value(value.id())?;
        Ok(match value {
            RgbValue::Brightness(_) => RgbValue::Brightness(data[0]),
            RgbValue::Effect(_) => RgbValue::Effect(data[0]),
            RgbValue::Speed(_) => RgbValue::Speed(data[0]),
            RgbValue::Color { .. } => RgbValue::Color {
                hue: data[0],
                saturation: data[1],
            },
        })
    }

    /// `id_custom_get_value` on the RGB matrix channel: returns the value bytes.
    fn lighting_value(&mut self, value_id: u8) -> Result<[u8; 2], ViaError> {
        let reply = self.exchange(Command::CustomGetValue, &[RGB_MATRIX_CHANNEL, value_id])?;
        check_lighting_echo(&reply, value_id)?;
        Ok([reply[3], reply[4]])
    }
}

fn check_lighting_echo(reply: &[u8], value_id: u8) -> Result<(), ViaError> {
    if reply[1] != RGB_MATRIX_CHANNEL || reply[2] != value_id {
        return Err(ViaError::WrongLightingValue {
            expected: RGB_MATRIX_CHANNEL,
            value: value_id,
            channel: reply[1],
            actual: reply[2],
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use openkeychron_hid::{DeviceIdentity, Event, Fixture, ReplayTransport};

    use super::{RgbMatrix, RgbValue, brightness_to_store, stored_brightness};
    use crate::client::{Via, ViaError};
    use crate::command::REPORT_LEN;

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

    /// The firmware echoes a set or save request unchanged.
    fn echoed(request: &[u8]) -> Vec<Event> {
        exchange(request, request)
    }

    fn via(sessions: &[Vec<Event>]) -> Via<ReplayTransport> {
        let device = DeviceIdentity {
            name: "synthetic".to_owned(),
            vendor_id: 0x3434,
            product_id: 0x0951,
            usage_page: 0xff60,
            usage: 0x61,
        };
        Via::new(ReplayTransport::new(Fixture::new(
            device,
            sessions.concat(),
        )))
    }

    #[test]
    fn rgb_reads_brightness_effect_speed_and_colour() {
        let mut via = via(&[
            exchange(&[0x08, 3, 1], &[0x08, 3, 1, 254]),
            exchange(&[0x08, 3, 2], &[0x08, 3, 2, 6]),
            exchange(&[0x08, 3, 3], &[0x08, 3, 3, 127]),
            exchange(&[0x08, 3, 4], &[0x08, 3, 4, 170, 255]),
        ]);
        assert_eq!(
            via.rgb_matrix(),
            Ok(RgbMatrix {
                brightness: 254,
                effect: 6,
                speed: 127,
                hue: 170,
                saturation: 255,
            })
        );
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn rgb_brightness_is_saved_after_its_scaled_read_back_matches() {
        let mut via = via(&[
            echoed(&[0x07, 3, 1, 128]),
            exchange(&[0x08, 3, 1], &[0x08, 3, 1, 127]),
            echoed(&[0x09, 3]),
        ]);
        assert_eq!(via.set_rgb_value(RgbValue::Brightness(128)), Ok(()));
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn rgb_colour_is_saved_after_its_read_back_matches() {
        let mut via = via(&[
            echoed(&[0x07, 3, 4, 85, 200]),
            exchange(&[0x08, 3, 4], &[0x08, 3, 4, 85, 200]),
            echoed(&[0x09, 3]),
        ]);
        assert_eq!(
            via.set_rgb_value(RgbValue::Color {
                hue: 85,
                saturation: 200
            }),
            Ok(())
        );
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn rgb_effect_is_saved_after_its_read_back_matches() {
        let mut via = via(&[
            echoed(&[0x07, 3, 2, 6]),
            exchange(&[0x08, 3, 2], &[0x08, 3, 2, 6]),
            echoed(&[0x09, 3]),
        ]);
        assert_eq!(via.set_rgb_value(RgbValue::Effect(6)), Ok(()));
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn rgb_speed_is_saved_after_its_read_back_matches() {
        let mut via = via(&[
            echoed(&[0x07, 3, 3, 200]),
            exchange(&[0x08, 3, 3], &[0x08, 3, 3, 200]),
            echoed(&[0x09, 3]),
        ]);
        assert_eq!(via.set_rgb_value(RgbValue::Speed(200)), Ok(()));
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn rgb_is_not_saved_when_the_effect_reads_back_clamped() {
        // An effect past the firmware's last one is clamped, here to 30.
        let mut via = via(&[
            echoed(&[0x07, 3, 2, 99]),
            exchange(&[0x08, 3, 2], &[0x08, 3, 2, 30]),
        ]);
        assert!(matches!(
            via.set_rgb_value(RgbValue::Effect(99)),
            Err(ViaError::ReadBackMismatch { .. })
        ));
        // No save request was sent: the fixture ends after the read-back.
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn rgb_reports_a_firmware_without_the_lighting_channel() {
        let mut via = via(&[exchange(&[0x08, 3, 1], &[0xff, 3, 1])]);
        assert_eq!(via.rgb_matrix(), Err(ViaError::Unhandled { command: 0x08 }));
    }

    #[test]
    fn rgb_rejects_a_reply_for_another_value() {
        let mut via = via(&[exchange(&[0x08, 3, 1], &[0x08, 3, 2, 6])]);
        assert!(matches!(
            via.rgb_matrix(),
            Err(ViaError::WrongLightingValue { .. })
        ));
    }

    #[test]
    fn rgb_brightness_scaling_matches_the_firmware() {
        assert_eq!(stored_brightness(255), 254);
        assert_eq!(stored_brightness(128), 127);
        assert_eq!(stored_brightness(0), 0);
    }

    #[test]
    fn rgb_brightness_to_store_inverts_the_scaling() {
        for stored in 0..=254u8 {
            let value = brightness_to_store(stored).unwrap();
            assert_eq!(stored_brightness(value), stored);
        }
        assert_eq!(brightness_to_store(255), None);
    }
}
