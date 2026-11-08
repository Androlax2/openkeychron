//! Keymap writes on the V5 Max ISO, each guarded by a strict snapshot of every layer's matrix
//! and encoder keys. Not a whole-device or whole-EEPROM backup.

use openkeychron_hid::{HidTransport, KeychronDevice};
use openkeychron_via::{EncoderDirection, Keycode, RegisteredModel, V5_MAX_FR_ISO, Via};
use serde::{Deserialize, Serialize};

use crate::via::{ViaReader, write_via};
use crate::{
    GuardError, GuardedWriteError, WriteVerified, write_verified_model, write_verified_models_text,
};

pub const BACKUP_VERSION: u32 = 1;
pub const LAYOUT_ID: &str = "v5-max-fr-iso";

/// The keyboard a backup was taken from.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BackupDevice {
    pub vendor_id: u16,
    pub product_id: u16,
}

impl From<&KeychronDevice> for BackupDevice {
    fn from(device: &KeychronDevice) -> Self {
        Self {
            vendor_id: device.vendor_id,
            product_id: device.product_id,
        }
    }
}

impl BackupDevice {
    /// The model this backup names, or why it is refused: a backup is only ever restored by
    /// writing it, so its model must be verified for writes.
    pub fn write_verified_model(&self) -> Result<RegisteredModel, GuardError> {
        write_verified_model(self.vendor_id, self.product_id).ok_or_else(|| {
            GuardError::UnverifiedBackupModel {
                vendor_id: self.vendor_id,
                product_id: self.product_id,
                models: write_verified_models_text(),
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EncoderPair {
    pub clockwise: u16,
    pub counter_clockwise: u16,
}

/// Every layer's matrix and encoder keycodes.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KeymapBackup {
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
    /// Checks the backup is complete and for the supported profile. Every write checks it before
    /// it reads the keyboard, so a caller that loads a backup checks it early only for a better
    /// error.
    pub fn validate(&self) -> Result<(), GuardError> {
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
}

fn invalid(error: impl std::fmt::Display) -> GuardError {
    GuardError::InvalidKeymapBackup(error.to_string())
}

/// A matrix position with a physical key in the supported V5 Max ISO layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyPosition {
    row: u8,
    column: u8,
}

impl KeyPosition {
    pub fn new(row: u8, column: u8) -> Result<Self, GuardError> {
        if V5_MAX_FR_ISO.key(row, column).is_none() {
            return Err(GuardError::NoPhysicalKey { row, column });
        }
        Ok(Self { row, column })
    }
}

/// Sets one key on `layer` to `keycode`, then requires the whole keymap to read back as the
/// backup with that one change. Returns the keymap from before the change.
pub fn set_keycode<T, E>(
    via: &mut Via<T>,
    verified: &WriteVerified,
    layer: u8,
    key: KeyPosition,
    keycode: u16,
    keep: impl FnOnce(&KeymapBackup) -> Result<(), E>,
) -> Result<KeymapBackup, GuardedWriteError<KeymapBackup, E>>
where
    T: HidTransport,
    E: From<GuardError>,
{
    write_via(
        via,
        verified,
        |reader| {
            let previous = capture(reader, verified.device(), None)?;
            if layer >= previous.layer_count {
                return Err(GuardError::LayerUnavailable {
                    layer,
                    last_layer: previous.layer_count - 1,
                }
                .into());
            }
            Ok((previous, ()))
        },
        keep,
        |via, previous, ()| {
            let mut expected = previous.clone();
            let offset = (usize::from(layer) * usize::from(expected.rows) + usize::from(key.row))
                * usize::from(expected.columns)
                + usize::from(key.column);
            expected.matrix[offset] = keycode;
            via.write_keycode(layer, key.row, key.column, Keycode::from(keycode))
                .map_err(GuardError::from)?;
            Ok(verify(via, verified.device(), &expected)?)
        },
    )
}

/// Writes `wanted`'s matrix and encoder keys, then requires the keymap to read back as `wanted`.
/// Returns the keymap it replaced, the undo of the restore.
pub fn restore<T, E>(
    via: &mut Via<T>,
    verified: &WriteVerified,
    wanted: &KeymapBackup,
    keep: impl FnOnce(&KeymapBackup) -> Result<(), E>,
) -> Result<KeymapBackup, GuardedWriteError<KeymapBackup, E>>
where
    T: HidTransport,
    E: From<GuardError>,
{
    wanted
        .validate()
        .map_err(|error| GuardedWriteError::NothingWritten(error.into()))?;
    write_via(
        via,
        verified,
        |reader| Ok((capture(reader, verified.device(), Some(wanted))?, ())),
        keep,
        |via, _previous, ()| Ok(write_whole_keymap(via, verified.device(), wanted)?),
    )
}

fn write_whole_keymap<T: HidTransport>(
    via: &mut Via<T>,
    device: &KeychronDevice,
    wanted: &KeymapBackup,
) -> Result<(), GuardError> {
    let matrix: Vec<_> = wanted.matrix.iter().copied().map(Keycode::from).collect();
    via.write_keymap(&matrix)?;
    for layer in 0..wanted.layer_count {
        let pair = &wanted.encoders[usize::from(layer)];
        via.write_encoder_keycode(
            layer,
            0,
            EncoderDirection::Clockwise,
            Keycode::from(pair.clockwise),
        )?;
        via.write_encoder_keycode(
            layer,
            0,
            EncoderDirection::CounterClockwise,
            Keycode::from(pair.counter_clockwise),
        )?;
    }
    verify(via, device, wanted)
}

/// Resets every layer's matrix and encoder keys to the firmware defaults. The defaults are
/// unknown, so the returned keymap from before the reset is its only undo.
pub fn reset<T, E>(
    via: &mut Via<T>,
    verified: &WriteVerified,
    keep: impl FnOnce(&KeymapBackup) -> Result<(), E>,
) -> Result<KeymapBackup, GuardedWriteError<KeymapBackup, E>>
where
    T: HidTransport,
    E: From<GuardError>,
{
    write_via(
        via,
        verified,
        |reader| Ok((capture(reader, verified.device(), None)?, ())),
        keep,
        |via, previous, ()| {
            via.reset_keymap().map_err(GuardError::from)?;
            capture(&mut ViaReader::new(via), verified.device(), Some(previous))?;
            Ok(())
        },
    )
}

/// Reads every layer's matrix and encoder keys. With `expected`, the keyboard must report the
/// same layer count and layout options, so a restore never writes a keymap laid out for other
/// firmware.
fn capture<T: HidTransport>(
    via: &mut ViaReader<'_, T>,
    device: &KeychronDevice,
    expected: Option<&KeymapBackup>,
) -> Result<KeymapBackup, GuardError> {
    let layer_count = via.layer_count()?;
    let layout_options = via.layout_options()?;
    if layout_options != 0
        || expected.is_some_and(|backup| {
            backup.layer_count != layer_count || backup.layout_options != layout_options
        })
    {
        return Err(GuardError::KeymapProfileMismatch);
    }
    let matrix = via
        .keymap(
            usize::from(layer_count)
                * usize::from(V5_MAX_FR_ISO.rows)
                * usize::from(V5_MAX_FR_ISO.columns),
        )?
        .into_iter()
        .map(u16::from)
        .collect();
    let mut encoders = Vec::with_capacity(usize::from(layer_count));
    // The pinned V5 Max ISO definition has exactly one physical encoder, at index zero.
    for layer in 0..layer_count {
        encoders.push(EncoderPair {
            clockwise: u16::from(via.encoder_keycode(layer, 0, EncoderDirection::Clockwise)?),
            counter_clockwise: u16::from(via.encoder_keycode(
                layer,
                0,
                EncoderDirection::CounterClockwise,
            )?),
        });
    }
    let backup = KeymapBackup {
        format_version: BACKUP_VERSION,
        device: BackupDevice::from(device),
        layout: LAYOUT_ID.to_owned(),
        layout_options,
        rows: V5_MAX_FR_ISO.rows,
        columns: V5_MAX_FR_ISO.columns,
        layer_count,
        encoder_count: 1,
        matrix,
        encoders,
    };
    backup.validate()?;
    Ok(backup)
}

/// The read-back: the whole keymap must equal `expected`.
fn verify<T: HidTransport>(
    via: &mut Via<T>,
    device: &KeychronDevice,
    expected: &KeymapBackup,
) -> Result<(), GuardError> {
    if capture(&mut ViaReader::new(via), device, Some(expected))? != *expected {
        return Err(GuardError::KeymapReadBackMismatch);
    }
    Ok(())
}
