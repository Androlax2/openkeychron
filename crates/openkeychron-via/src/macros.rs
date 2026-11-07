//! The dynamic macro buffer: NUL-separated macros, each a mix of ASCII text and QMK
//! `send_string` actions. Layout from `dynamic_keymap_macro_send` in
//! <https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/dynamic_keymap.c>
//! and the action codes and parser in
//! <https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/send_string/send_string_keycodes.h>
//! and `send_string_with_delay_impl` in
//! <https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/send_string/send_string.c>.

use openkeychron_hid::HidTransport;
use thiserror::Error;

use crate::command::{Command, REPORT_LEN};
use crate::{Via, ViaError};

/// `size <= 28` in `id_dynamic_keymap_macro_get_buffer` and `_set_buffer` (`quantum/via.c`).
const CHUNK_BYTES: u8 = 28;

/// `SS_QMK_PREFIX`: the byte that starts an action instead of a text character.
const PREFIX: u8 = 1;
const TAP: u8 = 1;
const DOWN: u8 = 2;
const UP: u8 = 3;
const DELAY: u8 = 4;
/// The firmware ends a delay at its first non-digit byte and skips that byte; VIA writes `|`.
const DELAY_END: u8 = b'|';

/// One step of a macro. Keycodes are 8-bit basic HID keycodes: the firmware's
/// `send_string` taps, presses and releases `uint8_t` codes only. Text is typed as US-ANSI
/// keycodes, so the host's keyboard layout decides which characters appear.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroAction {
    /// ASCII characters `0x02..=0x7f`, typed one after another.
    Text(String),
    Tap(u8),
    Down(u8),
    Up(u8),
    /// Milliseconds.
    Delay(u16),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MacroError {
    #[error("macro byte 0x{byte:02x} at offset {offset} is neither ASCII text nor an action")]
    InvalidByte { offset: usize, byte: u8 },

    #[error(
        "macro action at offset {offset} has unknown code {code} (known: 1 tap, 2 down, 3 up, 4 delay)"
    )]
    UnknownAction { offset: usize, code: u8 },

    #[error("macro action at offset {offset} ends before its keycode")]
    Truncated { offset: usize },

    #[error(
        "macro delay at offset {offset} is not 0–65535 ms written as plain digits ending in '|'"
    )]
    InvalidDelay { offset: usize },

    #[error("a macro keycode must not be 0: the firmware reads 0 as the end of the macro")]
    ZeroKeycode,

    #[error("macro text must not be empty")]
    EmptyText,

    #[error("macro text can only hold ASCII characters 0x02–0x7f, got {character:?}")]
    InvalidText { character: char },

    #[error("macro slot {slot} contains a 0 byte, which would end it early")]
    ZeroInSlot { slot: usize },

    #[error("the macros need {needed} bytes but the keyboard's macro buffer holds {size}")]
    TooLong { needed: usize, size: usize },
}

/// Reads one macro, as stored between two NUL separators. Rejects anything that would not
/// round-trip through [`encode_macro`] byte for byte, so callers can keep the raw bytes of
/// a macro they cannot show.
pub fn decode_macro(bytes: &[u8]) -> Result<Vec<MacroAction>, MacroError> {
    let mut actions = Vec::new();
    let mut offset = 0;
    while let Some(&byte) = bytes.get(offset) {
        if byte != PREFIX {
            if !is_text_byte(byte) {
                return Err(MacroError::InvalidByte { offset, byte });
            }
            match actions.last_mut() {
                Some(MacroAction::Text(text)) => text.push(char::from(byte)),
                _ => actions.push(MacroAction::Text(char::from(byte).to_string())),
            }
            offset += 1;
            continue;
        }
        let code = *bytes
            .get(offset + 1)
            .ok_or(MacroError::Truncated { offset })?;
        let (action, length) = match code {
            TAP | DOWN | UP => {
                let key = *bytes
                    .get(offset + 2)
                    .ok_or(MacroError::Truncated { offset })?;
                if key == 0 {
                    return Err(MacroError::InvalidByte {
                        offset: offset + 2,
                        byte: key,
                    });
                }
                let action = match code {
                    TAP => MacroAction::Tap(key),
                    DOWN => MacroAction::Down(key),
                    _ => MacroAction::Up(key),
                };
                (action, 3)
            }
            DELAY => {
                let digits = &bytes[offset + 2..];
                let count = digits
                    .iter()
                    .take_while(|byte| byte.is_ascii_digit())
                    .count();
                let milliseconds = parse_delay(&digits[..count], digits.get(count))
                    .ok_or(MacroError::InvalidDelay { offset })?;
                (MacroAction::Delay(milliseconds), 2 + count + 1)
            }
            _ => return Err(MacroError::UnknownAction { offset, code }),
        };
        actions.push(action);
        offset += length;
    }
    Ok(actions)
}

/// Writes one macro without its NUL separator.
pub fn encode_macro(actions: &[MacroAction]) -> Result<Vec<u8>, MacroError> {
    let mut bytes = Vec::new();
    for action in actions {
        match action {
            MacroAction::Text(text) => {
                if text.is_empty() {
                    return Err(MacroError::EmptyText);
                }
                for character in text.chars() {
                    let byte = u8::try_from(character)
                        .ok()
                        .filter(|byte| is_text_byte(*byte))
                        .ok_or(MacroError::InvalidText { character })?;
                    bytes.push(byte);
                }
            }
            MacroAction::Tap(key) => bytes.extend(key_action(TAP, *key)?),
            MacroAction::Down(key) => bytes.extend(key_action(DOWN, *key)?),
            MacroAction::Up(key) => bytes.extend(key_action(UP, *key)?),
            MacroAction::Delay(milliseconds) => {
                bytes.extend([PREFIX, DELAY]);
                bytes.extend(milliseconds.to_string().bytes());
                bytes.push(DELAY_END);
            }
        }
    }
    Ok(bytes)
}

/// The first `count` macros in `buffer`. A macro the buffer ends before is empty; bytes after
/// the last counted separator are ignored, as the firmware ignores them.
#[must_use]
pub fn slots(buffer: &[u8], count: u8) -> Vec<Vec<u8>> {
    let mut parts = buffer.split(|byte| *byte == 0);
    (0..count)
        .map(|_| parts.next().unwrap_or_default().to_vec())
        .collect()
}

/// Joins macros with NUL separators and pads with zeros to `size`. The last byte stays zero:
/// the firmware plays no macro while it is non-zero.
pub fn buffer_from_slots(slots: &[Vec<u8>], size: usize) -> Result<Vec<u8>, MacroError> {
    if let Some(slot) = slots.iter().position(|bytes| bytes.contains(&0)) {
        return Err(MacroError::ZeroInSlot { slot });
    }
    let mut buffer = slots.join(&0);
    while buffer.last() == Some(&0) {
        buffer.pop();
    }
    let needed = buffer.len() + 1;
    if needed > size {
        return Err(MacroError::TooLong { needed, size });
    }
    buffer.resize(size, 0);
    Ok(buffer)
}

impl<T: HidTransport> Via<T> {
    /// `id_dynamic_keymap_macro_get_count` (`0x0C`): how many macros the buffer can hold, not
    /// how many are stored.
    pub fn macro_count(&mut self) -> Result<u8, ViaError> {
        Ok(self.exchange(Command::GetMacroCount, &[])?[1])
    }

    /// `id_dynamic_keymap_macro_get_buffer_size` (`0x0D`), in bytes. Zero is refused: the
    /// firmware's last byte is the write-in-progress flag, so a buffer needs at least one.
    pub fn macro_buffer_size(&mut self) -> Result<u16, ViaError> {
        let reply = self.exchange(Command::GetMacroBufferSize, &[])?;
        match u16::from_be_bytes([reply[1], reply[2]]) {
            0 => Err(ViaError::EmptyMacroBuffer),
            size => Ok(size),
        }
    }

    /// `id_dynamic_keymap_macro_get_buffer` (`0x0E`): the first `size` bytes of the buffer.
    /// Only reads.
    pub fn macro_buffer(&mut self, size: u16) -> Result<Vec<u8>, ViaError> {
        let mut buffer = Vec::with_capacity(usize::from(size));
        for start in (0..size).step_by(usize::from(CHUNK_BYTES)) {
            buffer.extend(self.read_macro_chunk(start, chunk_len(size - start))?);
        }
        Ok(buffer)
    }

    /// Writes: replaces the whole macro buffer with `buffer`, which must end in 0 and be
    /// exactly the size the keyboard reports (asked first, before any write). Follows the firmware's rule in
    /// `quantum/dynamic_keymap.h`: set the last byte to `0xFF` (macros disabled), write the
    /// rest, then set it back to 0. Every write is read back. Save a backup first: an error
    /// can leave the buffer half written, with every macro disabled.
    pub fn write_macro_buffer(&mut self, buffer: &[u8]) -> Result<(), ViaError> {
        let Some((&0, body)) = buffer.split_last() else {
            return Err(ViaError::InvalidMacroBuffer {
                length: buffer.len(),
            });
        };
        let keyboard = self.macro_buffer_size()?;
        if usize::from(keyboard) != buffer.len() {
            return Err(ViaError::MacroBufferSizeMismatch {
                keyboard,
                buffer: buffer.len(),
            });
        }
        let last = keyboard - 1;
        self.write_macro_chunk(last, &[0xff])?;
        for start in (0..last).step_by(usize::from(CHUNK_BYTES)) {
            let end = start + u16::from(chunk_len(last - start));
            self.write_macro_chunk(start, &body[usize::from(start)..usize::from(end)])?;
        }
        self.write_macro_chunk(last, &[0])
    }

    fn read_macro_chunk(&mut self, offset: u16, size: u8) -> Result<Vec<u8>, ViaError> {
        let [high, low] = offset.to_be_bytes();
        let reply = self.exchange(Command::GetMacroBuffer, &[high, low, size])?;
        validate_macro_chunk(&reply, Command::GetMacroBuffer, offset, size)?;
        Ok(reply[4..4 + usize::from(size)].to_vec())
    }

    /// `bytes` holds at most [`CHUNK_BYTES`].
    fn write_macro_chunk(&mut self, offset: u16, bytes: &[u8]) -> Result<(), ViaError> {
        let size = u8::try_from(bytes.len()).map_err(|_| ViaError::InvalidMacroBuffer {
            length: bytes.len(),
        })?;
        let [high, low] = offset.to_be_bytes();
        let request = [&[high, low, size][..], bytes].concat();
        let reply = self.exchange(Command::SetMacroBuffer, &request)?;
        validate_macro_chunk(&reply, Command::SetMacroBuffer, offset, size)?;
        let read_back = self.read_macro_chunk(offset, size)?;
        for ((offset, &written), read) in (offset..).zip(bytes).zip(read_back) {
            if read != written {
                return Err(ViaError::MacroWriteMismatch {
                    offset,
                    written,
                    read,
                });
            }
        }
        Ok(())
    }
}

fn chunk_len(remaining: u16) -> u8 {
    u8::try_from(remaining).map_or(CHUNK_BYTES, |remaining| remaining.min(CHUNK_BYTES))
}

fn validate_macro_chunk(
    reply: &[u8; REPORT_LEN],
    command: Command,
    offset: u16,
    size: u8,
) -> Result<(), ViaError> {
    let actual_offset = u16::from_be_bytes([reply[1], reply[2]]);
    if actual_offset != offset || reply[3] != size {
        return Err(ViaError::WrongMacroChunk {
            command: command.id(),
            expected_offset: offset,
            actual_offset,
            expected_size: size,
            actual_size: reply[3],
        });
    }
    Ok(())
}

const fn is_text_byte(byte: u8) -> bool {
    matches!(byte, 0x02..=0x7f)
}

fn key_action(code: u8, key: u8) -> Result<[u8; 3], MacroError> {
    if key == 0 {
        return Err(MacroError::ZeroKeycode);
    }
    Ok([PREFIX, code, key])
}

/// Only the form [`encode_macro`] writes: plain digits, no leading zero, then `|`.
fn parse_delay(digits: &[u8], end: Option<&u8>) -> Option<u16> {
    if end != Some(&DELAY_END) || digits.is_empty() || (digits.len() > 1 && digits[0] == b'0') {
        return None;
    }
    std::str::from_utf8(digits).ok()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::{MacroAction, MacroError, buffer_from_slots, decode_macro, encode_macro, slots};

    #[test]
    fn decodes_a_macro_into_text_taps_keys_and_delays_and_encodes_the_same_bytes() {
        let cases: [(&[u8], Vec<MacroAction>); 6] = [
            (b"hello", vec![MacroAction::Text("hello".to_owned())]),
            (&[1, 1, 0x28], vec![MacroAction::Tap(0x28)]),
            (&[1, 2, 0xe1], vec![MacroAction::Down(0xe1)]),
            (&[1, 3, 0xe1], vec![MacroAction::Up(0xe1)]),
            (b"\x01\x04250|", vec![MacroAction::Delay(250)]),
            (
                b"a\x01\x041|7\x01\x01\x28",
                vec![
                    MacroAction::Text("a".to_owned()),
                    MacroAction::Delay(1),
                    MacroAction::Text("7".to_owned()),
                    MacroAction::Tap(0x28),
                ],
            ),
        ];
        for (bytes, actions) in cases {
            assert_eq!(decode_macro(bytes), Ok(actions.clone()), "{bytes:02x?}");
            assert_eq!(encode_macro(&actions), Ok(bytes.to_vec()), "{actions:?}");
        }
    }

    #[test]
    fn an_empty_macro_has_no_actions() {
        assert_eq!(decode_macro(&[]), Ok(vec![]));
        assert_eq!(encode_macro(&[]), Ok(vec![]));
    }

    #[test]
    fn decoding_a_macro_rejects_what_the_firmware_would_not_play_as_written() {
        let cases: [(&[u8], MacroError); 9] = [
            (b"a\0", MacroError::InvalidByte { offset: 1, byte: 0 }),
            (&[1, 1, 0], MacroError::InvalidByte { offset: 2, byte: 0 }),
            (&[1, 5, 4], MacroError::UnknownAction { offset: 0, code: 5 }),
            (&[1], MacroError::Truncated { offset: 0 }),
            (&[1, 1], MacroError::Truncated { offset: 0 }),
            (b"\x01\x04", MacroError::InvalidDelay { offset: 0 }),
            (b"\x01\x0412", MacroError::InvalidDelay { offset: 0 }),
            (b"x\x01\x04012|", MacroError::InvalidDelay { offset: 1 }),
            (
                b"ok\x80",
                MacroError::InvalidByte {
                    offset: 2,
                    byte: 0x80,
                },
            ),
        ];
        for (bytes, error) in cases {
            assert_eq!(decode_macro(bytes), Err(error), "{bytes:02x?}");
        }
    }

    #[test]
    fn decoding_a_macro_rejects_a_delay_beyond_sixteen_bits() {
        assert_eq!(
            decode_macro(b"\x01\x0465536|"),
            Err(MacroError::InvalidDelay { offset: 0 })
        );
    }

    #[test]
    fn encoding_a_macro_rejects_actions_that_would_break_the_buffer() {
        let cases = [
            (MacroAction::Tap(0), MacroError::ZeroKeycode),
            (MacroAction::Down(0), MacroError::ZeroKeycode),
            (MacroAction::Up(0), MacroError::ZeroKeycode),
            (MacroAction::Text(String::new()), MacroError::EmptyText),
            (
                MacroAction::Text("é".to_owned()),
                MacroError::InvalidText { character: 'é' },
            ),
            (
                MacroAction::Text("a\u{1}".to_owned()),
                MacroError::InvalidText { character: '\u{1}' },
            ),
        ];
        for (action, error) in cases {
            assert_eq!(
                encode_macro(std::slice::from_ref(&action)),
                Err(error),
                "{action:?}"
            );
        }
    }

    #[test]
    fn splits_a_macro_buffer_into_the_reported_number_of_slots() {
        let buffer = b"ab\0\0c\0junk\0\0\0";
        assert_eq!(
            slots(buffer, 4),
            vec![b"ab".to_vec(), vec![], b"c".to_vec(), b"junk".to_vec()]
        );
        assert_eq!(slots(buffer, 2), vec![b"ab".to_vec(), vec![]]);
    }

    #[test]
    fn macro_slots_missing_from_the_buffer_are_empty() {
        assert_eq!(slots(b"ab\0\0\0", 3), vec![b"ab".to_vec(), vec![], vec![]]);
        assert_eq!(slots(b"abc", 2), vec![b"abc".to_vec(), vec![]]);
    }

    #[test]
    fn joins_macro_slots_into_a_zero_padded_buffer() {
        let joined = buffer_from_slots(&[b"ab".to_vec(), vec![], b"c".to_vec(), vec![]], 8);
        assert_eq!(joined, Ok(b"ab\0\0c\0\0\0".to_vec()));
    }

    #[test]
    fn macro_slots_may_fill_the_buffer_up_to_its_final_zero() {
        let joined = buffer_from_slots(&[b"ab".to_vec(), b"cd".to_vec(), vec![]], 6);
        assert_eq!(joined, Ok(b"ab\0cd\0".to_vec()));
    }

    #[test]
    fn rejects_a_macro_slot_containing_the_zero_separator() {
        assert_eq!(
            buffer_from_slots(&[b"a".to_vec(), b"b\0c".to_vec()], 8),
            Err(MacroError::ZeroInSlot { slot: 1 })
        );
    }

    #[test]
    fn rejects_macro_slots_that_do_not_fit_the_buffer() {
        assert_eq!(
            buffer_from_slots(&[b"ab".to_vec(), b"cde".to_vec()], 6),
            Err(MacroError::TooLong { needed: 7, size: 6 })
        );
    }
}
