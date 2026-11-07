//! What every backup-guarded keymap and macro write shares: the supported-keyboard check,
//! the session around the write, and the failure and success messages naming the backup.

use std::path::Path;

use openkeychron_hid::KeychronDevice;
use openkeychron_via::Via;
use serde::Serialize;

use crate::session::{Session, SessionError};

/// Whether `device` is the V5 Max ISO on a USB cable, the only keyboard whose layout is known.
/// A receiver does not identify the keyboard behind it.
pub(crate) fn is_v5_max_iso_cable(device: &KeychronDevice) -> bool {
    !device.is_receiver && device.vendor_id == 0x3434 && device.product_id == 0x0951
}

/// The wording of one kind of backup-guarded write.
pub(crate) struct WriteKind {
    /// The command that writes and restores this state: `keymap` or `macro`.
    pub(crate) command: &'static str,
    /// The error when the device is not [`is_v5_max_iso_cable`].
    pub(crate) cable_refusal: &'static str,
    /// What a write that fails after the backup can leave behind.
    pub(crate) failure_risk: &'static str,
    /// What the pre-change backup holds.
    pub(crate) backup_contents: &'static str,
}

/// The new file that receives the pre-change state before the first write is sent.
pub(crate) struct PreChangeBackup<'a> {
    path: &'a Path,
    is_saved: bool,
}

impl PreChangeBackup<'_> {
    /// Saves the pre-change state with `save_new`; from then on, a failure names the backup.
    pub(crate) fn save(
        &mut self,
        save_new: impl FnOnce(&Path) -> Result<(), SessionError>,
    ) -> Result<(), SessionError> {
        save_new(self.path)?;
        self.is_saved = true;
        Ok(())
    }
}

/// Runs `write` against a supported cable keyboard and ends the session. Once `write` has
/// saved the pre-change backup, every failure, an unfinished replay included, names that
/// backup and the command that restores it.
pub(crate) fn run(
    kind: &WriteKind,
    fixture: Option<&Path>,
    device: Option<&Path>,
    backup_path: &Path,
    write: impl FnOnce(
        &mut Via<Session>,
        &KeychronDevice,
        &mut PreChangeBackup<'_>,
    ) -> Result<(), SessionError>,
) -> Result<(), SessionError> {
    let session = Session::start(fixture, None, device)?;
    let identity = session.device().clone();
    let mut via = Via::new(session);
    let mut backup = PreChangeBackup {
        path: backup_path,
        is_saved: false,
    };
    let result = if is_v5_max_iso_cable(&identity) {
        write(&mut via, &identity, &mut backup)
    } else {
        Err(SessionError::Usage(kind.cable_refusal.to_owned()))
    };
    via.into_transport()
        .complete(result)
        .map_err(|error| failure(kind, error, &backup))
}

fn failure(kind: &WriteKind, error: SessionError, backup: &PreChangeBackup<'_>) -> SessionError {
    if !backup.is_saved {
        return error;
    }
    let path = backup.path.display();
    SessionError::Usage(format!(
        "{error}; {command} state may have changed, and {risk}; \
         pre-change {contents} backup retained at {path}; \
         undo with `openkeychron {command} restore {path} --backup <NEW_FILE>`",
        command = kind.command,
        risk = kind.failure_risk,
        contents = kind.backup_contents,
    ))
}

#[derive(Debug, Serialize)]
struct SuccessOutput<'a> {
    operation: &'static str,
    backup: &'a Path,
}

pub(crate) fn print_success(
    kind: &WriteKind,
    operation: &'static str,
    summary: &str,
    backup: &Path,
    json: bool,
) -> Result<(), SessionError> {
    if json {
        return crate::print_json(&SuccessOutput { operation, backup });
    }
    println!("{summary}");
    println!("Pre-change backup: {}", backup.display());
    println!(
        "Undo with: openkeychron {} restore {} --backup <NEW_FILE>",
        kind.command,
        backup.display()
    );
    Ok(())
}
