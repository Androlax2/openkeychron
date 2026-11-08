//! Backup-before-write editing for the explicitly supported cable keymap profile.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use clap::Args;
use openkeychron_guard::keymap::{self, KeymapBackup};

use crate::backup_file;
use crate::guarded_write::{self, WriteKind};
use crate::session::SessionError;

const KEYMAP_WRITE: WriteKind = WriteKind {
    command: "keymap",
    writes: "keymap writes",
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
    let key = keymap::KeyPosition::new(args.key.row, args.key.column)?;
    guarded_write::run(
        &KEYMAP_WRITE,
        fixture,
        device,
        &args.backup,
        |via, verified, keep| {
            keymap::set_keycode(via, verified, args.layer, key, args.keycode, keep)
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
    let wanted = load_backup(&args.source)?;
    guarded_write::run(
        &KEYMAP_WRITE,
        fixture,
        device,
        &args.backup,
        |via, verified, keep| keymap::restore(via, verified, &wanted, keep),
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
        |via, verified, keep| keymap::reset(via, verified, keep),
    )?;
    guarded_write::print_success(
        &KEYMAP_WRITE,
        "reset",
        "Keymap reset to the firmware's default matrix and encoder keys.",
        &args.backup,
        json,
    )
}

/// Loads and checks a keymap backup before any device is opened.
fn load_backup(path: &Path) -> Result<KeymapBackup, SessionError> {
    let backup: KeymapBackup = backup_file::load(path, "keymap")?;
    backup.validate()?;
    Ok(backup)
}
