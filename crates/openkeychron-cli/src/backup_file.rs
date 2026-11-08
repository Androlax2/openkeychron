//! Backup files written before any keymap, macro or RGB write.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::session::SessionError;

const MAX_BACKUP_BYTES: usize = 1_048_576;

/// Saves `value` (a `kind` backup, named in errors) as pretty JSON to a file that must not exist yet, and syncs it, so a
/// failure here happens before the caller sends any write to the device.
pub(crate) fn save_new(
    path: &Path,
    kind: &str,
    value: &impl Serialize,
) -> Result<(), SessionError> {
    let mut text = serde_json::to_string_pretty(value)
        .map_err(|error| SessionError::Usage(format!("cannot encode {kind} backup: {error}")))?;
    text.push('\n');
    let failure = |error| {
        SessionError::Usage(format!(
            "cannot save new {kind} backup {}: {error}; choose a new writable --backup path; \
             no write was sent to the keyboard",
            path.display()
        ))
    };
    let mut file = File::options()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(failure)?;
    file.write_all(text.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(failure)
}

/// Saves `value` like [`save_new`], then reads the file back and requires it to equal
/// `value`, so a write is only sent once the undo is known to load.
pub(crate) fn save_new_verified<T>(path: &Path, kind: &str, value: &T) -> Result<(), SessionError>
where
    T: Serialize + DeserializeOwned + PartialEq,
{
    save_new(path, kind, value)?;
    let unverified = |reason: &dyn std::fmt::Display| {
        SessionError::Usage(format!(
            "new {kind} backup {} did not read back: {reason}; no write was sent to the keyboard",
            path.display()
        ))
    };
    let saved: T = load(path, kind).map_err(|error| unverified(&error))?;
    if saved != *value {
        return Err(unverified(&"its content differs from the captured state"));
    }
    Ok(())
}

/// Reads a JSON backup of at most 1 MiB. Callers validate the content themselves.
pub(crate) fn load<T: DeserializeOwned>(path: &Path, kind: &str) -> Result<T, SessionError> {
    let invalid = |error: &dyn std::fmt::Display| {
        SessionError::Usage(format!("invalid {kind} backup {}: {error}", path.display()))
    };
    let file = File::open(path).map_err(|error| {
        SessionError::Usage(format!(
            "cannot read {kind} backup {}: {error}",
            path.display()
        ))
    })?;
    let read_limit = u64::try_from(MAX_BACKUP_BYTES + 1).map_err(|error| invalid(&error))?;
    let mut text = String::new();
    file.take(read_limit)
        .read_to_string(&mut text)
        .map_err(|error| invalid(&error))?;
    if text.len() > MAX_BACKUP_BYTES {
        return Err(invalid(&"file exceeds 1 MiB"));
    }
    serde_json::from_str(&text).map_err(|error| invalid(&error))
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::save_new_verified;

    /// Equal to nothing, so every read-back comparison fails.
    #[derive(Serialize, Deserialize)]
    struct NeverEqual;

    impl PartialEq for NeverEqual {
        fn eq(&self, _: &Self) -> bool {
            false
        }
    }

    #[test]
    fn refuses_a_backup_that_does_not_read_back_identically() {
        let directory =
            std::env::temp_dir().join(format!("openkeychron-backup-file-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("backup.json");
        let error = save_new_verified(&path, "test", &NeverEqual).unwrap_err();
        std::fs::remove_dir_all(&directory).unwrap();
        assert!(
            error.to_string().contains("did not read back")
                && error.to_string().contains("no write was sent"),
            "{error}"
        );
    }
}
