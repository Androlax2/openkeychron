//! Backup-before-write editing for the explicitly supported cable keymap profile.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use clap::Args;
use openkeychron_hid::KeychronDevice;
use openkeychron_via::{EncoderDirection, Keycode, V5_MAX_FR_ISO, Via};

use crate::guarded_write::{self, WriteKind};
use crate::keymap_backup::{BACKUP_VERSION, BackupDevice, EncoderPair, KeymapBackup, LAYOUT_ID};
use crate::session::{Session, SessionError};

const KEYMAP_WRITE: WriteKind = WriteKind {
    command: "keymap",
    cable_refusal: "keymap writes require the V5 Max ISO (3434:0951) on a USB cable; receiver \
                    and unknown-layout writes are refused",
    failure_risk: "complete restoration was not confirmed",
    backup_contents: "matrix/encoder",
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct KeyPosition {
    row: u8,
    column: u8,
}

impl FromStr for KeyPosition {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let failure =
            || format!("expected ROW,COLUMN as zero-based byte-sized coordinates, got {value:?}");
        let (row, column) = value.split_once(',').ok_or_else(failure)?;
        Ok(Self {
            row: row.parse().map_err(|_| failure())?,
            column: column.parse().map_err(|_| failure())?,
        })
    }
}

#[derive(Debug, Args)]
pub(crate) struct SetArgs {
    layer: u8,
    /// Zero-based matrix position, ROW,COLUMN (see keymap show).
    key: KeyPosition,
    /// Raw 16-bit decimal or 0x-prefixed hexadecimal keycode.
    #[arg(value_parser = parse_keycode)]
    keycode: u16,
    /// New file for the complete pre-change matrix and encoder keymap.
    #[arg(long, value_name = "FILE", conflicts_with = "record")]
    backup: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct RestoreArgs {
    /// Complete versioned matrix/encoder backup to restore.
    #[arg(value_name = "BACKUP")]
    source: PathBuf,
    /// New, separate file for the complete pre-restore keymap.
    #[arg(long, value_name = "FILE", conflicts_with = "record")]
    backup: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct ResetArgs {
    /// New file for the complete pre-reset matrix and encoder keymap.
    #[arg(long, value_name = "FILE", conflicts_with = "record")]
    backup: PathBuf,
}

fn parse_keycode(value: &str) -> Result<u16, String> {
    let (digits, radix) = match value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        Some(digits) => (digits, 16),
        None => (value, 10),
    };
    u16::from_str_radix(digits, radix)
        .map_err(|_| format!("expected a 16-bit decimal or hexadecimal keycode, got {value:?}"))
}

pub(crate) fn set(
    args: &SetArgs,
    fixture: Option<&Path>,
    device: Option<&Path>,
    json: bool,
) -> Result<(), SessionError> {
    if V5_MAX_FR_ISO.key(args.key.row, args.key.column).is_none() {
        return Err(SessionError::Usage(format!(
            "no physical key at ({}, {}) in the supported V5 Max ISO layout",
            args.key.row, args.key.column
        )));
    }
    guarded_write::run(
        &KEYMAP_WRITE,
        fixture,
        device,
        &args.backup,
        |via, identity, backup| {
            let mut expected = capture(via, identity, None)?;
            if args.layer >= expected.layer_count {
                return Err(SessionError::Usage(format!(
                    "layer {} is unavailable; select 0..={}",
                    args.layer,
                    expected.layer_count - 1
                )));
            }
            backup.save(|path| expected.save_new(path))?;
            let offset = (usize::from(args.layer) * usize::from(expected.rows)
                + usize::from(args.key.row))
                * usize::from(expected.columns)
                + usize::from(args.key.column);
            expected.matrix[offset] = args.keycode;
            via.write_keycode(
                args.layer,
                args.key.row,
                args.key.column,
                Keycode::from(args.keycode),
            )
            .map_err(|error| SessionError::via(&error, false))?;
            verify(via, identity, &expected)
        },
    )?;
    guarded_write::print_success(
        &KEYMAP_WRITE,
        "set",
        "Keymap set verified against the complete matrix/encoder snapshot.",
        &args.backup,
        json,
    )
}

pub(crate) fn restore(
    args: &RestoreArgs,
    fixture: Option<&Path>,
    device: Option<&Path>,
    json: bool,
) -> Result<(), SessionError> {
    let wanted = KeymapBackup::load(&args.source)?;
    guarded_write::run(
        &KEYMAP_WRITE,
        fixture,
        device,
        &args.backup,
        |via, identity, backup| {
            let previous = capture(via, identity, Some(&wanted))?;
            backup.save(|path| previous.save_new(path))?;
            let matrix: Vec<_> = wanted.matrix.iter().copied().map(Keycode::from).collect();
            via.write_keymap(&matrix)
                .map_err(|error| SessionError::via(&error, false))?;
            for layer in 0..wanted.layer_count {
                let pair = &wanted.encoders[usize::from(layer)];
                via.write_encoder_keycode(
                    layer,
                    0,
                    EncoderDirection::Clockwise,
                    Keycode::from(pair.clockwise),
                )
                .map_err(|error| SessionError::via(&error, false))?;
                via.write_encoder_keycode(
                    layer,
                    0,
                    EncoderDirection::CounterClockwise,
                    Keycode::from(pair.counter_clockwise),
                )
                .map_err(|error| SessionError::via(&error, false))?;
            }
            verify(via, identity, &wanted)
        },
    )?;
    guarded_write::print_success(
        &KEYMAP_WRITE,
        "restore",
        "Keymap restore verified against the complete matrix/encoder snapshot.",
        &args.backup,
        json,
    )
}

pub(crate) fn reset(
    args: &ResetArgs,
    fixture: Option<&Path>,
    device: Option<&Path>,
    json: bool,
) -> Result<(), SessionError> {
    guarded_write::run(
        &KEYMAP_WRITE,
        fixture,
        device,
        &args.backup,
        |via, identity, backup| {
            let previous = capture(via, identity, None)?;
            // The defaults are unknown, so this verified backup is the only undo.
            backup.save(|path| previous.save_new(path))?;
            via.reset_keymap()
                .map_err(|error| SessionError::via(&error, false))?;
            capture(via, identity, Some(&previous)).map(|_| ())
        },
    )?;
    guarded_write::print_success(
        &KEYMAP_WRITE,
        "reset",
        "Keymap reset to the firmware's default matrix and encoder keys.",
        &args.backup,
        json,
    )
}

fn capture(
    via: &mut Via<Session>,
    device: &KeychronDevice,
    expected: Option<&KeymapBackup>,
) -> Result<KeymapBackup, SessionError> {
    let layer_count = via
        .layer_count()
        .map_err(|error| SessionError::via(&error, false))?;
    let layout_options = via
        .layout_options()
        .map_err(|error| SessionError::via(&error, false))?;
    if layout_options != 0
        || expected.is_some_and(|backup| {
            backup.layer_count != layer_count || backup.layout_options != layout_options
        })
    {
        return Err(SessionError::Usage(
            "target layout/options/layer count do not match the supported backup profile"
                .to_owned(),
        ));
    }
    let matrix = via
        .keymap(
            usize::from(layer_count)
                * usize::from(V5_MAX_FR_ISO.rows)
                * usize::from(V5_MAX_FR_ISO.columns),
        )
        .map_err(|error| SessionError::via(&error, false))?
        .into_iter()
        .map(u16::from)
        .collect();
    let mut encoders = Vec::with_capacity(usize::from(layer_count));
    // The pinned V5 Max ISO definition has exactly one physical encoder, at index zero.
    for layer in 0..layer_count {
        encoders.push(EncoderPair {
            clockwise: u16::from(
                via.encoder_keycode(layer, 0, EncoderDirection::Clockwise)
                    .map_err(|error| SessionError::via(&error, false))?,
            ),
            counter_clockwise: u16::from(
                via.encoder_keycode(layer, 0, EncoderDirection::CounterClockwise)
                    .map_err(|error| SessionError::via(&error, false))?,
            ),
        });
    }
    let backup = KeymapBackup {
        format_version: BACKUP_VERSION,
        device: BackupDevice {
            vendor_id: device.vendor_id,
            product_id: device.product_id,
        },
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

fn verify(
    via: &mut Via<Session>,
    device: &KeychronDevice,
    expected: &KeymapBackup,
) -> Result<(), SessionError> {
    if capture(via, device, Some(expected))? != *expected {
        return Err(SessionError::Usage(
            "complete matrix/encoder keymap verification did not match the expected state"
                .to_owned(),
        ));
    }
    Ok(())
}
