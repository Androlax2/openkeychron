//! Macro writes, each guarded by a strict snapshot of the whole raw macro buffer. Not a backup
//! of keymaps or other settings.

use openkeychron_hid::{HidTransport, KeychronDevice};
use openkeychron_via::{Via, buffer_from_slots, slots};
use serde::{Deserialize, Serialize};

use crate::keymap::BackupDevice;
use crate::via::{ViaReader, write_via};
use crate::{GuardError, GuardedWriteError, WriteVerified};

pub const BACKUP_VERSION: u32 = 1;

/// The whole macro buffer.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MacroBackup {
    pub format_version: u32,
    pub device: BackupDevice,
    pub macro_count: u8,
    pub buffer_size: u16,
    /// Every byte, including any after the last macro, so a restore is exact.
    pub buffer: Vec<u8>,
}

impl MacroBackup {
    /// Checks the backup is complete and for a model verified for writes. Every write checks it
    /// before it reads the keyboard, so a caller that loads a backup checks it early only for a
    /// better error.
    pub fn validate(&self) -> Result<(), GuardError> {
        if self.format_version != BACKUP_VERSION {
            return Err(invalid("unsupported format_version"));
        }
        self.device.write_verified_model().map_err(invalid)?;
        if self.buffer_size == 0 || self.buffer.len() != usize::from(self.buffer_size) {
            return Err(invalid("buffer length does not match buffer_size"));
        }
        Ok(())
    }

    /// The firmware plays no macro while the buffer's last byte is non-zero.
    #[must_use]
    pub fn macros_enabled(&self) -> bool {
        self.buffer.last() == Some(&0)
    }
}

fn invalid(error: impl std::fmt::Display) -> GuardError {
    GuardError::InvalidMacroBackup(error.to_string())
}

/// Replaces macro `slot` with `encoded` (from `openkeychron_via::encode_macro`), then requires
/// the whole buffer to read back as the backup with that one change. Returns the buffer from
/// before the change.
pub fn set_macro<T, E>(
    via: &mut Via<T>,
    verified: &WriteVerified,
    slot: u8,
    encoded: &[u8],
    keep: impl FnOnce(&MacroBackup) -> Result<(), E>,
) -> Result<MacroBackup, GuardedWriteError<MacroBackup, E>>
where
    T: HidTransport,
    E: From<GuardError>,
{
    write_via(
        via,
        verified,
        |reader| {
            let previous = capture(reader, verified.device(), None)?;
            let buffer = buffer_with_macro(&previous, slot, encoded)?;
            Ok((previous, buffer))
        },
        keep,
        |via, previous, buffer| {
            via.write_macro_buffer(&buffer).map_err(GuardError::from)?;
            let expected = MacroBackup {
                buffer,
                ..previous.clone()
            };
            Ok(verify(via, verified.device(), &expected)?)
        },
    )
}

/// The buffer `previous` holds with macro `slot` replaced, or why it cannot be written.
fn buffer_with_macro(
    previous: &MacroBackup,
    slot: u8,
    encoded: &[u8],
) -> Result<Vec<u8>, GuardError> {
    if !previous.macros_enabled() {
        return Err(GuardError::MacrosDisabled);
    }
    if previous.macro_count == 0 {
        return Err(GuardError::NoMacroSlots);
    }
    if slot >= previous.macro_count {
        return Err(GuardError::MacroSlotUnavailable {
            slot,
            last_slot: previous.macro_count - 1,
        });
    }
    let mut macros = slots(&previous.buffer, previous.macro_count);
    macros[usize::from(slot)] = encoded.to_vec();
    Ok(buffer_from_slots(&macros, previous.buffer.len())?)
}

/// Writes `wanted`'s whole buffer, then requires it to read back as `wanted`. Returns the buffer
/// it replaced, the undo of the restore.
pub fn restore<T, E>(
    via: &mut Via<T>,
    verified: &WriteVerified,
    wanted: &MacroBackup,
    keep: impl FnOnce(&MacroBackup) -> Result<(), E>,
) -> Result<MacroBackup, GuardedWriteError<MacroBackup, E>>
where
    T: HidTransport,
    E: From<GuardError>,
{
    let refused = |error: GuardError| GuardedWriteError::NothingWritten(error.into());
    wanted.validate().map_err(refused)?;
    if !wanted.macros_enabled() {
        return Err(refused(GuardError::RestoredMacrosDisabled));
    }
    write_via(
        via,
        verified,
        |reader| Ok((capture(reader, verified.device(), Some(wanted))?, ())),
        keep,
        |via, _previous, ()| {
            via.write_macro_buffer(&wanted.buffer)
                .map_err(GuardError::from)?;
            Ok(verify(via, verified.device(), wanted)?)
        },
    )
}

/// Reads the whole macro buffer. With `expected`, the keyboard must report the same macro
/// count and buffer size, so a restore never writes a buffer laid out for other firmware.
fn capture<T: HidTransport>(
    via: &mut ViaReader<'_, T>,
    device: &KeychronDevice,
    expected: Option<&MacroBackup>,
) -> Result<MacroBackup, GuardError> {
    let macro_count = via.macro_count()?;
    let buffer_size = via.macro_buffer_size()?;
    if expected.is_some_and(|backup| {
        backup.macro_count != macro_count || backup.buffer_size != buffer_size
    }) {
        return Err(GuardError::MacroLayoutMismatch {
            macro_count,
            buffer_size,
        });
    }
    let backup = MacroBackup {
        format_version: BACKUP_VERSION,
        device: BackupDevice::from(device),
        macro_count,
        buffer_size,
        buffer: via.macro_buffer(buffer_size)?,
    };
    backup.validate()?;
    Ok(backup)
}

/// The read-back: the whole buffer must equal `expected`.
fn verify<T: HidTransport>(
    via: &mut Via<T>,
    device: &KeychronDevice,
    expected: &MacroBackup,
) -> Result<(), GuardError> {
    if capture(&mut ViaReader::new(via), device, Some(expected))? != *expected {
        return Err(GuardError::MacroReadBackMismatch);
    }
    Ok(())
}
