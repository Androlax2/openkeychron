//! Guarded Keychron settings writes: debounce, Snap Click and wireless timers. Each setter in
//! `openkeychron-keychron` reads its value back and fails on a mismatch.

use openkeychron_hid::HidTransport;
use openkeychron_keychron::{Debounce, Keychron, KeychronError, SnapClickPair, WirelessTimers};

use crate::{GuardedWriteError, WriteVerified, keep_then_write};

/// The settings reads a settings write needs before its backup, and no write.
pub struct SettingsReader<'a, T>(&'a mut Keychron<T>);

impl<T: HidTransport> SettingsReader<'_, T> {
    pub fn debounce(&mut self) -> Result<Debounce, KeychronError> {
        self.0.debounce()
    }

    pub fn snap_click_pairs(&mut self) -> Result<Vec<SnapClickPair>, KeychronError> {
        self.0.snap_click_pairs()
    }

    pub fn wireless_timers(&mut self) -> Result<WirelessTimers, KeychronError> {
        self.0.wireless_timers()
    }
}

/// Runs a settings write: `capture` reads the previous value, `keep` stores it, and only then
/// does `write` get the [`Keychron`] client. Returns the previous value.
pub fn write_settings<T, Backup, Plan, E>(
    keychron: &mut Keychron<T>,
    _verified: &WriteVerified,
    capture: impl FnOnce(&mut SettingsReader<'_, T>) -> Result<(Backup, Plan), E>,
    keep: impl FnOnce(&Backup) -> Result<(), E>,
    write: impl FnOnce(&mut Keychron<T>, &Backup, Plan) -> Result<(), E>,
) -> Result<Backup, GuardedWriteError<Backup, E>>
where
    T: HidTransport,
{
    let (backup, plan) =
        capture(&mut SettingsReader(keychron)).map_err(GuardedWriteError::NothingWritten)?;
    keep_then_write(backup, keep, |backup| write(keychron, backup, plan))
}
