//! Strict snapshots of the whole raw macro buffer, not of keymaps or other settings.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::backup_file;
use crate::guarded_write::PreChangeSnapshot;
use crate::keymap_backup::BackupDevice;
use crate::session::SessionError;

pub(crate) const BACKUP_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MacroBackup {
    pub format_version: u32,
    pub device: BackupDevice,
    pub macro_count: u8,
    pub buffer_size: u16,
    /// Every byte, including any after the last macro, so a restore is exact.
    pub buffer: Vec<u8>,
}

impl MacroBackup {
    pub fn validate(&self) -> Result<(), SessionError> {
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
    pub fn macros_enabled(&self) -> bool {
        self.buffer.last() == Some(&0)
    }

    pub fn load(path: &Path) -> Result<Self, SessionError> {
        let backup: Self = backup_file::load(path, "macro")?;
        backup.validate()?;
        Ok(backup)
    }
}

impl PreChangeSnapshot for MacroBackup {
    fn save_new(&self, path: &Path) -> Result<(), SessionError> {
        self.validate()?;
        backup_file::save_new_verified(path, "macro", self)
    }
}

fn invalid(error: impl std::fmt::Display) -> SessionError {
    SessionError::Usage(format!("invalid macro backup: {error}"))
}
