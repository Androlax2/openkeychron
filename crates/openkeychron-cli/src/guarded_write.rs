//! What the CLI adds to every keymap and macro write from `openkeychron_guard`: the session
//! around the write, the new backup file that keeps the pre-change state, and the failure and
//! success messages naming that file.

use std::path::Path;

use openkeychron_guard::{GuardedWriteError, WriteVerified, require_write_verified};
use openkeychron_via::Via;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::backup_file;
use crate::session::{Session, SessionError};

/// The wording of one kind of backup-guarded write.
pub(crate) struct WriteKind {
    /// The command that writes and restores this state, and names its backup: `keymap` or
    /// `macro`.
    pub(crate) command: &'static str,
    /// What is written, for [`require_write_verified`]: `keymap writes`.
    pub(crate) writes: &'static str,
    /// What a write that fails after the backup can leave behind.
    pub(crate) failure_risk: &'static str,
    /// What the pre-change backup holds.
    pub(crate) backup_contents: &'static str,
}

/// How the CLI keeps a backup: in a new file, read back before anything is written.
pub(crate) type KeepInFile<'a, Backup> = &'a dyn Fn(&Backup) -> Result<(), SessionError>;

/// Runs a guarded write against a supported cable keyboard and ends the session. `write` calls
/// one of the `openkeychron_guard` writes with the [`WriteVerified`] device and a `keep` step
/// that saves the backup to `backup_path`. From the save on, every failure, an unfinished replay
/// included, names that backup and the command that restores it.
pub(crate) fn run<Backup>(
    kind: &WriteKind,
    fixture: Option<&Path>,
    device: Option<&Path>,
    backup_path: &Path,
    write: impl FnOnce(
        &mut Via<Session>,
        &WriteVerified,
        KeepInFile<'_, Backup>,
    ) -> Result<Backup, GuardedWriteError<Backup, SessionError>>,
) -> Result<(), SessionError>
where
    Backup: Serialize + DeserializeOwned + PartialEq,
{
    let session = Session::start(fixture, None, device)?;
    let identity = session.device().clone();
    let mut via = Via::new(session);
    let keep = |backup: &Backup| backup_file::save_new_verified(backup_path, kind.command, backup);
    let (result, is_backup_kept) = match require_write_verified(kind.writes, &identity) {
        Err(refusal) => (Err(refusal.into()), false),
        Ok(verified) => match write(&mut via, &verified, &keep) {
            Ok(_) => (Ok(()), true),
            Err(GuardedWriteError::NothingWritten(error)) => (Err(error), false),
            Err(GuardedWriteError::AfterBackup { error, .. }) => (Err(error), true),
        },
    };
    via.into_transport().complete(result).map_err(|error| {
        if is_backup_kept {
            failure(kind, &error, backup_path)
        } else {
            error
        }
    })
}

fn failure(kind: &WriteKind, error: &SessionError, backup: &Path) -> SessionError {
    let path = backup.display();
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

/// Prints `summary` with the backup and its undo command, or `operation` and the backup as JSON.
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
