//! Strict keymap snapshots, not whole-device or whole-EEPROM backups.

use std::path::Path;

use openkeychron_via::V5_MAX_FR_ISO;
use serde::{Deserialize, Serialize};

use crate::backup_file;
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
        let backup: Self = backup_file::load(path, "keymap")?;
        backup.validate()?;
        Ok(backup)
    }

    pub fn save_new(&self, path: &Path) -> Result<(), SessionError> {
        self.validate()?;
        backup_file::save_new_verified(path, "keymap", self)
    }
}

fn invalid(error: impl std::fmt::Display) -> SessionError {
    SessionError::Usage(format!("invalid keymap backup: {error}"))
}
