//! Strict keymap snapshots, not whole-device or whole-EEPROM backups.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use openkeychron_via::V5_MAX_FR_ISO;
use serde::{Deserialize, Serialize};

use crate::session::SessionError;

pub(crate) const BACKUP_VERSION: u32 = 1;
pub(crate) const LAYOUT_ID: &str = "v5-max-fr-iso";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BackupDevice {
    pub vendor_id: u16,
    pub product_id: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EncoderPair {
    pub clockwise: u16,
    pub counter_clockwise: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct KeymapBackup {
    pub format_version: u32,
    pub device: BackupDevice,
    pub layout: String,
    pub layout_options: u32,
    pub rows: u8,
    pub columns: u8,
    pub layer_count: u8,
    pub encoder_count: u8,
    pub matrix: Vec<u16>,
    pub encoders: Vec<EncoderPair>,
}

impl KeymapBackup {
    pub fn validate(&self) -> Result<(), SessionError> {
        if self.format_version != BACKUP_VERSION {
            return Err(invalid("unsupported format_version"));
        }
        if self.device.vendor_id != 0x3434 || self.device.product_id != 0x0951 {
            return Err(invalid(
                "device identity does not match the cable V5 Max ISO",
            ));
        }
        if self.layout != LAYOUT_ID
            || self.layout_options != 0
            || self.rows != V5_MAX_FR_ISO.rows
            || self.columns != V5_MAX_FR_ISO.columns
            || self.encoder_count != 1
            || self.layer_count == 0
        {
            return Err(invalid(
                "layout/options/dimensions/encoder count do not match the supported profile",
            ));
        }
        let matrix_size =
            usize::from(self.layer_count) * usize::from(self.rows) * usize::from(self.columns);
        if self.matrix.len() != matrix_size || self.encoders.len() != usize::from(self.layer_count)
        {
            return Err(invalid("incomplete matrix or encoder assignments"));
        }
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, SessionError> {
        const MAX_BACKUP_BYTES: usize = 1_048_576;
        let file = File::open(path).map_err(|error| {
            SessionError::Usage(format!(
                "cannot read keymap backup {}: {error}",
                path.display()
            ))
        })?;
        let read_limit = u64::try_from(MAX_BACKUP_BYTES + 1)
            .map_err(|error| invalid(format!("invalid backup size limit: {error}")))?;
        let mut text = String::new();
        file.take(read_limit)
            .read_to_string(&mut text)
            .map_err(|error| invalid(format!("cannot read {}: {error}", path.display())))?;
        if text.len() > MAX_BACKUP_BYTES {
            return Err(invalid("file exceeds 1 MiB"));
        }
        let backup: Self = serde_json::from_str(&text).map_err(invalid)?;
        backup.validate()?;
        Ok(backup)
    }

    pub fn save_new(&self, path: &Path) -> Result<(), SessionError> {
        self.validate()?;
        let mut text = serde_json::to_string_pretty(self).map_err(invalid)?;
        text.push('\n');
        let failure = |error| {
            SessionError::Usage(format!(
                "cannot save new keymap backup {}: {error}; choose a new writable --backup path; \
             no keymap write was attempted",
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
}

fn invalid(error: impl std::fmt::Display) -> SessionError {
    SessionError::Usage(format!("invalid keymap backup: {error}"))
}
