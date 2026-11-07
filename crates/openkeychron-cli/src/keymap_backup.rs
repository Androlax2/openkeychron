//! Strict keymap snapshots, not whole-device or whole-EEPROM backups.

use std::path::Path;

use openkeychron_via::RegisteredModel;
use serde::{Deserialize, Serialize};

use crate::backup_file;
use crate::guarded_write::{self, PreChangeSnapshot};
use crate::session::SessionError;

pub(crate) const BACKUP_VERSION: u32 = 1;
pub(crate) const LAYOUT_ID: &str = "v5-max-fr-iso";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BackupDevice {
    pub vendor_id: u16,
    pub product_id: u16,
}

impl BackupDevice {
    /// The model this backup names, or why it is refused: a backup is only ever restored by
    /// writing it, so its model must be verified for writes.
    pub fn write_verified_model(&self) -> Result<RegisteredModel, String> {
        guarded_write::write_verified_model(self.vendor_id, self.product_id).ok_or_else(|| {
            format!(
                "device {:04x}:{:04x} is not a model verified for writes; supported: {}",
                self.vendor_id,
                self.product_id,
                guarded_write::write_verified_models_text()
            )
        })
    }
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
        let model = self.device.write_verified_model().map_err(invalid)?;
        if self.layout != LAYOUT_ID
            || self.layout_options != 0
            || self.rows != model.layout().rows()
            || self.columns != model.layout().columns()
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
}

impl PreChangeSnapshot for KeymapBackup {
    fn save_new(&self, path: &Path) -> Result<(), SessionError> {
        self.validate()?;
        backup_file::save_new_verified(path, "keymap", self)
    }
}

fn invalid(error: impl std::fmt::Display) -> SessionError {
    SessionError::Usage(format!("invalid keymap backup: {error}"))
}
