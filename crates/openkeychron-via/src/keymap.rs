//! Read-only dynamic keymap commands, from `id_dynamic_keymap_get_layer_count` and
//! `id_dynamic_keymap_get_buffer` in
//! <https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/via.c>.

use openkeychron_hid::HidTransport;

use crate::command::Command;
use crate::{Keycode, Via, ViaError};

const CHUNK_BYTES: usize = 28;
const MAX_KEYS: usize = 32_768;

impl<T: HidTransport> Via<T> {
    /// Reads the number of dynamic keymap layers. A zero-layer reply is invalid.
    pub fn layer_count(&mut self) -> Result<u8, ViaError> {
        let count = self.exchange(Command::GetLayerCount, &[])?[1];
        if count == 0 {
            return Err(ViaError::InvalidLayerCount { count });
        }
        Ok(count)
    }

    /// Reads `key_count` matrix entries from byte offset zero, in layer/row/column order.
    ///
    /// The caller must supply the actual matrix entry count (layers × rows × columns),
    /// including unused positions, from the device definition. VIA cannot detect a count
    /// that exceeds the keyboard's real keymap: firmware may return zeros beyond it.
    /// Encoder maps are separate and are not read here. No keyboard settings are changed.
    pub fn keymap(&mut self, key_count: usize) -> Result<Vec<Keycode>, ViaError> {
        if !(1..=MAX_KEYS).contains(&key_count) {
            return Err(ViaError::InvalidKeymapSize { key_count });
        }
        let total_bytes = key_count * 2;
        let mut keys = Vec::with_capacity(key_count);
        for start in (0..total_bytes).step_by(CHUNK_BYTES) {
            let offset =
                u16::try_from(start).map_err(|_| ViaError::InvalidKeymapSize { key_count })?;
            let size = u8::try_from((total_bytes - start).min(CHUNK_BYTES))
                .map_err(|_| ViaError::InvalidKeymapSize { key_count })?;
            let [high, low] = offset.to_be_bytes();
            let reply = self.exchange(Command::GetKeymapBuffer, &[high, low, size])?;
            let actual_offset = u16::from_be_bytes([reply[1], reply[2]]);
            if actual_offset != offset || reply[3] != size {
                return Err(ViaError::WrongKeymapChunk {
                    expected_offset: offset,
                    actual_offset,
                    expected_size: size,
                    actual_size: reply[3],
                });
            }
            keys.extend(
                reply[4..4 + usize::from(size)]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|bytes| Keycode::from(u16::from_be_bytes(*bytes))),
            );
        }
        Ok(keys)
    }
}
