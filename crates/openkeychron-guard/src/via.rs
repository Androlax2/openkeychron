//! Guarded VIA writes: keymaps and macros.

use openkeychron_hid::HidTransport;
use openkeychron_via::{EncoderDirection, Keycode, Via, ViaError};

use crate::{GuardedWriteError, WriteVerified, keep_then_write};

/// The VIA reads a capture needs and no write: [`write_via`] lends the capture step only this,
/// so no write can reach the keyboard before the backup is kept.
pub struct ViaReader<'a, T>(&'a mut Via<T>);

impl<'a, T: HidTransport> ViaReader<'a, T> {
    pub(crate) fn new(via: &'a mut Via<T>) -> Self {
        Self(via)
    }

    pub fn layer_count(&mut self) -> Result<u8, ViaError> {
        self.0.layer_count()
    }

    pub fn layout_options(&mut self) -> Result<u32, ViaError> {
        self.0.layout_options()
    }

    pub fn keymap(&mut self, key_count: usize) -> Result<Vec<Keycode>, ViaError> {
        self.0.keymap(key_count)
    }

    pub fn encoder_keycode(
        &mut self,
        layer: u8,
        encoder: u8,
        direction: EncoderDirection,
    ) -> Result<Keycode, ViaError> {
        self.0.encoder_keycode(layer, encoder, direction)
    }

    pub fn macro_count(&mut self) -> Result<u8, ViaError> {
        self.0.macro_count()
    }

    pub fn macro_buffer_size(&mut self) -> Result<u16, ViaError> {
        self.0.macro_buffer_size()
    }

    pub fn macro_buffer(&mut self, size: u16) -> Result<Vec<u8>, ViaError> {
        self.0.macro_buffer(size)
    }
}

/// Runs a VIA write on a write-verified device. `capture` reads the backup, and anything the
/// write needs, through a [`ViaReader`]; `keep` stores the backup; only then does `write` get the
/// [`Via`]. Returns the backup, which undoes the write.
pub fn write_via<T, Backup, Plan, E>(
    via: &mut Via<T>,
    _verified: &WriteVerified,
    capture: impl FnOnce(&mut ViaReader<'_, T>) -> Result<(Backup, Plan), E>,
    keep: impl FnOnce(&Backup) -> Result<(), E>,
    write: impl FnOnce(&mut Via<T>, &Backup, Plan) -> Result<(), E>,
) -> Result<Backup, GuardedWriteError<Backup, E>>
where
    T: HidTransport,
{
    let (backup, plan) =
        capture(&mut ViaReader::new(via)).map_err(GuardedWriteError::NothingWritten)?;
    keep_then_write(backup, keep, |backup| write(via, backup, plan))
}
