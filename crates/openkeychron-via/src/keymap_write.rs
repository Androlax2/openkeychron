//! Keymap reads and verified writes. Protocol layouts come from
//! <https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/via.c>.

use openkeychron_hid::HidTransport;

use crate::command::{Command, REPORT_LEN};
use crate::keymap::{CHUNK_BYTES, MAX_KEYS, validate_chunk};
use crate::{Keycode, Via, ViaError};

/// Direction in an encoder map; VIA uses 1 for clockwise, 0 for counter-clockwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncoderDirection {
    Clockwise,
    CounterClockwise,
}

impl EncoderDirection {
    const fn flag(self) -> u8 {
        match self {
            Self::Clockwise => 1,
            Self::CounterClockwise => 0,
        }
    }
}

impl<T: HidTransport> Via<T> {
    /// Reads one matrix assignment. The caller must validate the device's matrix bounds.
    pub fn keycode(&mut self, layer: u8, row: u8, column: u8) -> Result<Keycode, ViaError> {
        self.read_address(Command::GetKeycode, [layer, row, column])
    }

    /// Reads an encoder assignment. The caller must validate layer/encoder bounds.
    pub fn encoder_keycode(
        &mut self,
        layer: u8,
        encoder: u8,
        direction: EncoderDirection,
    ) -> Result<Keycode, ViaError> {
        self.read_address(Command::GetEncoder, [layer, encoder, direction.flag()])
    }

    /// Writes one matrix assignment and reads it back. The caller must validate bounds
    /// and save a backup first. An error does not imply that the device is unchanged.
    pub fn write_keycode(
        &mut self,
        layer: u8,
        row: u8,
        column: u8,
        keycode: Keycode,
    ) -> Result<(), ViaError> {
        self.write_address(
            Command::SetKeycode,
            Command::GetKeycode,
            [layer, row, column],
            keycode,
        )
    }

    /// Writes one encoder assignment and reads it back. Validate bounds and save a backup
    /// first; an error may leave changed state. Does not change pairing or receiver mapping.
    pub fn write_encoder_keycode(
        &mut self,
        layer: u8,
        encoder: u8,
        direction: EncoderDirection,
        keycode: Keycode,
    ) -> Result<(), ViaError> {
        self.write_address(
            Command::SetEncoder,
            Command::GetEncoder,
            [layer, encoder, direction.flag()],
            keycode,
        )
    }

    /// Writes the matrix buffer from offset zero, reading back each chunk. Save a complete
    /// backup first. The caller supplies actual matrix dimensions; no encoder map is included.
    /// An error may leave earlier chunks, or the failing chunk, changed.
    pub fn write_keymap(&mut self, keycodes: &[Keycode]) -> Result<(), ViaError> {
        let key_count = keycodes.len();
        if !(1..=MAX_KEYS).contains(&key_count) {
            return Err(ViaError::InvalidKeymapSize {
                key_count: keycodes.len(),
            });
        }
        for (chunk, keys) in keycodes.chunks(CHUNK_BYTES / 2).enumerate() {
            let start = chunk * CHUNK_BYTES;
            let offset =
                u16::try_from(start).map_err(|_| ViaError::InvalidKeymapSize { key_count })?;
            let size = u8::try_from(keys.len() * 2)
                .map_err(|_| ViaError::InvalidKeymapSize { key_count })?;
            let [high, low] = offset.to_be_bytes();
            let mut arguments = vec![high, low, size];
            arguments.extend(keys.iter().flat_map(|key| u16::from(*key).to_be_bytes()));
            let reply = self.exchange(Command::SetKeymapBuffer, &arguments)?;
            validate_chunk(&reply, offset, size)?;
            let reply = self.exchange(Command::GetKeymapBuffer, &[high, low, size])?;
            validate_chunk(&reply, offset, size)?;
            for (index, (key, bytes)) in keys
                .iter()
                .zip(reply[4..4 + usize::from(size)].as_chunks::<2>().0)
                .enumerate()
            {
                let written = u16::from(*key);
                let read = u16::from_be_bytes(*bytes);
                if read != written {
                    return Err(ViaError::KeymapWriteMismatch {
                        offset: u16::try_from(start + index * 2)
                            .map_err(|_| ViaError::InvalidKeymapSize { key_count })?,
                        written,
                        read,
                    });
                }
            }
        }
        Ok(())
    }

    fn read_address(&mut self, command: Command, address: [u8; 3]) -> Result<Keycode, ViaError> {
        let reply = self.exchange(command, &address)?;
        validate_address(&reply, command, address)?;
        Ok(Keycode::from(u16::from_be_bytes([reply[4], reply[5]])))
    }

    fn write_address(
        &mut self,
        set: Command,
        get: Command,
        address: [u8; 3],
        keycode: Keycode,
    ) -> Result<(), ViaError> {
        let written = u16::from(keycode);
        let [high, low] = written.to_be_bytes();
        let reply = self.exchange(set, &[address[0], address[1], address[2], high, low])?;
        validate_address(&reply, set, address)?;
        let read = u16::from(self.read_address(get, address)?);
        if read != written {
            return Err(ViaError::KeycodeWriteMismatch {
                command: set.id(),
                address,
                written,
                read,
            });
        }
        Ok(())
    }
}

fn validate_address(
    reply: &[u8; REPORT_LEN],
    command: Command,
    expected: [u8; 3],
) -> Result<(), ViaError> {
    let actual = [reply[1], reply[2], reply[3]];
    if actual != expected {
        return Err(ViaError::WrongKeymapAddress {
            command: command.id(),
            expected,
            actual,
        });
    }
    Ok(())
}
