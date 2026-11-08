//! The guarded write path every write to the keyboard takes, from the CLI or the app.
//!
//! 1. [`require_write_verified`] lets only a write-verified model on a USB cable through, and
//!    returns the [`WriteVerified`] proof every write below asks for.
//! 2. The capture step reads the state the write changes through a read-only wrapper
//!    ([`via::ViaReader`], [`rgb::LightingReader`], …), so it cannot send a write.
//! 3. The caller's `keep` step stores that backup: the CLI saves it to a new file, the app holds
//!    it in memory. A write is only sent once `keep` succeeds.
//! 4. The write step changes the keyboard and reads it back, failing on a mismatch.
//!
//! The backup is returned on success, and inside [`GuardedWriteError::AfterBackup`] when the
//! write fails, so the caller can always restore it, through the same path.

mod error;
mod gate;
pub mod keymap;
pub mod macros;
pub mod rgb;
pub mod settings;
pub mod via;

pub use error::{GuardError, GuardedWriteError};
pub use gate::{
    WriteVerified, require_write_verified, write_verified_model, write_verified_models_text,
};

/// The step every guarded write shares once its capture succeeded: `write` runs only after
/// `keep` stored the backup, and from then on a failure carries that backup.
fn keep_then_write<Backup, E>(
    backup: Backup,
    keep: impl FnOnce(&Backup) -> Result<(), E>,
    write: impl FnOnce(&Backup) -> Result<(), E>,
) -> Result<Backup, GuardedWriteError<Backup, E>> {
    keep(&backup).map_err(GuardedWriteError::NothingWritten)?;
    match write(&backup) {
        Ok(()) => Ok(backup),
        Err(error) => Err(GuardedWriteError::AfterBackup {
            error,
            backup: Box::new(backup),
        }),
    }
}
