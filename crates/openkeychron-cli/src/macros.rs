//! Macro commands. Writes replace the whole macro buffer, after saving it to a new backup.

use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use openkeychron_hid::KeychronDevice;
use openkeychron_via::{MacroAction, Via, buffer_from_slots, decode_macro, encode_macro, slots};
use serde::Serialize;

use crate::keymap_backup::BackupDevice;
use crate::macro_backup::{BACKUP_VERSION, MacroBackup};
use crate::session::{Session, SessionError};
use crate::{DeviceOutput, device_output};

#[derive(Debug, Subcommand)]
pub enum MacroCommand {
    /// Print every macro slot as actions. Only reads.
    List,
    /// Replace one macro after saving the whole macro buffer. Writes over USB cable only.
    ///
    /// Actions run in order: text:TEXT, tap:KEY, down:KEY, up:KEY, delay:MS. KEY is an 8-bit
    /// basic HID keycode, decimal or 0x hex. Text is typed as US-ANSI keycodes, so a French
    /// AZERTY host types `q` for text:a. No action empties the slot.
    Set(SetArgs),
    /// Restore a macro backup, saving a separate pre-restore backup first.
    Restore(RestoreArgs),
}

#[derive(Debug, Args)]
pub struct SetArgs {
    /// Zero-based macro slot (M0 is 0).
    slot: u8,
    #[arg(value_name = "ACTION", value_parser = parse_action)]
    actions: Vec<MacroAction>,
    /// New file for the whole pre-change macro buffer.
    #[arg(long, value_name = "FILE", conflicts_with = "record")]
    backup: PathBuf,
}

#[derive(Debug, Args)]
pub struct RestoreArgs {
    /// Macro backup to restore.
    #[arg(value_name = "BACKUP")]
    source: PathBuf,
    /// New, separate file for the pre-restore macro buffer.
    #[arg(long, value_name = "FILE", conflicts_with = "record")]
    backup: PathBuf,
}

fn parse_action(value: &str) -> Result<MacroAction, String> {
    let failure = || {
        format!(
            "expected text:TEXT, tap:KEY, down:KEY, up:KEY or delay:MS (KEY 1–255, MS 0–65535), \
             got {value:?}"
        )
    };
    let (kind, argument) = value.split_once(':').ok_or_else(failure)?;
    let key = || parse_key(argument).ok_or_else(failure);
    match kind {
        "text" => Ok(MacroAction::Text(argument.to_owned())),
        "tap" => Ok(MacroAction::Tap(key()?)),
        "down" => Ok(MacroAction::Down(key()?)),
        "up" => Ok(MacroAction::Up(key()?)),
        "delay" => argument
            .parse()
            .map(MacroAction::Delay)
            .map_err(|_| failure()),
        _ => Err(failure()),
    }
}

fn parse_key(value: &str) -> Option<u8> {
    match value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        Some(digits) => u8::from_str_radix(digits, 16).ok(),
        None => value.parse().ok(),
    }
}

#[derive(Debug, Serialize)]
struct ListOutput {
    device: DeviceOutput,
    macro_count: u8,
    buffer_size: u16,
    /// `false` when the buffer's last byte is non-zero: the firmware then plays no macro.
    macros_enabled: bool,
    macros: Vec<SlotOutput>,
}

#[derive(Debug, Serialize)]
struct SlotOutput {
    slot: usize,
    /// `null` when the bytes are not a macro this tool can read; see `error`.
    actions: Option<Vec<ActionOutput>>,
    error: Option<String>,
    bytes: Vec<u8>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum ActionOutput {
    Text(String),
    Tap(u8),
    Down(u8),
    Up(u8),
    Delay(u16),
}

impl From<MacroAction> for ActionOutput {
    fn from(action: MacroAction) -> Self {
        match action {
            MacroAction::Text(text) => Self::Text(text),
            MacroAction::Tap(key) => Self::Tap(key),
            MacroAction::Down(key) => Self::Down(key),
            MacroAction::Up(key) => Self::Up(key),
            MacroAction::Delay(milliseconds) => Self::Delay(milliseconds),
        }
    }
}

#[derive(Debug, Serialize)]
struct EditOutput<'a> {
    operation: &'static str,
    backup: &'a Path,
}

pub fn run(
    command: &MacroCommand,
    fixture: Option<&Path>,
    record: Option<&Path>,
    device: Option<&Path>,
    json: bool,
) -> Result<(), SessionError> {
    if record.is_some() && !matches!(command, MacroCommand::List) {
        return Err(SessionError::Usage(
            "--record is not supported for macro set/restore; it must not overwrite a protected backup".to_owned(),
        ));
    }
    match command {
        MacroCommand::List => list(Session::start(fixture, record, device)?, json),
        MacroCommand::Set(args) => set(args, fixture, device, json),
        MacroCommand::Restore(args) => restore(args, fixture, device, json),
    }
}

fn list(session: Session, json: bool) -> Result<(), SessionError> {
    let device = session.device().clone();
    let mut via = Via::new(session);
    let result = (|| {
        let via_error = |error| SessionError::via(&error, device.is_receiver);
        let macro_count = via.macro_count().map_err(via_error)?;
        let buffer_size = via.macro_buffer_size().map_err(via_error)?;
        let buffer = via.macro_buffer(buffer_size).map_err(via_error)?;
        let macros = slots(&buffer, macro_count)
            .into_iter()
            .enumerate()
            .map(|(slot, bytes)| {
                let (actions, error) = match decode_macro(&bytes) {
                    Ok(actions) => (Some(actions.into_iter().map(Into::into).collect()), None),
                    Err(error) => (None, Some(error.to_string())),
                };
                SlotOutput {
                    slot,
                    actions,
                    error,
                    bytes,
                }
            })
            .collect();
        Ok(ListOutput {
            device: device_output(&device),
            macro_count,
            buffer_size,
            macros_enabled: buffer.last() == Some(&0),
            macros,
        })
    })();
    let output = via.into_transport().complete(result)?;
    if json {
        return crate::print_json(&output);
    }
    print_list(&output);
    Ok(())
}

fn print_list(output: &ListOutput) {
    println!(
        "Macros: {} slots, {}-byte buffer. Keys are raw HID keycodes; text types US-ANSI keycodes.",
        output.macro_count, output.buffer_size
    );
    if !output.macros_enabled {
        println!(
            "Warning: the buffer's last byte is not 0, so the firmware plays no macro (an \
             earlier write did not finish). Fix it with `openkeychron macro restore`."
        );
    }
    for slot in &output.macros {
        let text = match (&slot.actions, &slot.error) {
            (Some(actions), _) if actions.is_empty() => "(empty)".to_owned(),
            (Some(actions), _) => actions
                .iter()
                .map(action_text)
                .collect::<Vec<_>>()
                .join(" "),
            (None, error) => format!(
                "unreadable: {}; raw bytes: {}",
                error.as_deref().unwrap_or("unknown error"),
                slot.bytes
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
        };
        println!("M{}  {text}", slot.slot);
    }
}

fn action_text(action: &ActionOutput) -> String {
    match action {
        ActionOutput::Text(text) => format!("text:{text:?}"),
        ActionOutput::Tap(key) => format!("tap:0x{key:02x}"),
        ActionOutput::Down(key) => format!("down:0x{key:02x}"),
        ActionOutput::Up(key) => format!("up:0x{key:02x}"),
        ActionOutput::Delay(milliseconds) => format!("delay:{milliseconds}"),
    }
}

fn set(
    args: &SetArgs,
    fixture: Option<&Path>,
    device: Option<&Path>,
    json: bool,
) -> Result<(), SessionError> {
    let encoded =
        encode_macro(&args.actions).map_err(|error| SessionError::Usage(error.to_string()))?;
    let session = Session::start(fixture, None, device)?;
    let identity = session.device().clone();
    let mut via = Via::new(session);
    let mut has_backup = false;
    let result = (|| {
        let previous = capture(&mut via, &identity, None)?;
        if !previous.macros_enabled() {
            return Err(SessionError::Usage(
                "the macro buffer's last byte is not 0, so an earlier write did not finish and \
                 its content is unknown; run `openkeychron macro restore` with a backup first"
                    .to_owned(),
            ));
        }
        if previous.macro_count == 0 {
            return Err(SessionError::Usage(
                "the keyboard reports no macro slots".to_owned(),
            ));
        }
        if args.slot >= previous.macro_count {
            return Err(SessionError::Usage(format!(
                "macro slot {} is unavailable; select 0..={}",
                args.slot,
                previous.macro_count - 1
            )));
        }
        let mut macros = slots(&previous.buffer, previous.macro_count);
        macros[usize::from(args.slot)] = encoded;
        let buffer = buffer_from_slots(&macros, previous.buffer.len())
            .map_err(|error| SessionError::Usage(error.to_string()))?;
        previous.save_new(&args.backup)?;
        has_backup = true;
        via.write_macro_buffer(&buffer)
            .map_err(|error| SessionError::via(&error, false))?;
        verify(&mut via, &identity, &MacroBackup { buffer, ..previous })
    })();
    via.into_transport()
        .complete(result)
        .map_err(|error| edit_failure(error, has_backup, &args.backup))?;
    print_success(
        "set",
        &format!(
            "Macro M{} set and the whole macro buffer verified.",
            args.slot
        ),
        &args.backup,
        json,
    )
}

fn restore(
    args: &RestoreArgs,
    fixture: Option<&Path>,
    device: Option<&Path>,
    json: bool,
) -> Result<(), SessionError> {
    let wanted = MacroBackup::load(&args.source)?;
    if !wanted.macros_enabled() {
        return Err(SessionError::Usage(format!(
            "macro backup {} records an unfinished write (last byte not 0); restoring it would \
             leave every macro disabled",
            args.source.display()
        )));
    }
    let session = Session::start(fixture, None, device)?;
    let identity = session.device().clone();
    let mut via = Via::new(session);
    let mut has_backup = false;
    let result = (|| {
        let previous = capture(&mut via, &identity, Some(&wanted))?;
        previous.save_new(&args.backup)?;
        has_backup = true;
        via.write_macro_buffer(&wanted.buffer)
            .map_err(|error| SessionError::via(&error, false))?;
        verify(&mut via, &identity, &wanted)
    })();
    via.into_transport()
        .complete(result)
        .map_err(|error| edit_failure(error, has_backup, &args.backup))?;
    print_success(
        "restore",
        "Macro buffer restored and verified.",
        &args.backup,
        json,
    )
}

/// Reads the whole macro buffer. With `expected`, the keyboard must report the same macro
/// count and buffer size, so a restore never writes a buffer laid out for other firmware.
fn capture(
    via: &mut Via<Session>,
    device: &KeychronDevice,
    expected: Option<&MacroBackup>,
) -> Result<MacroBackup, SessionError> {
    if device.is_receiver || device.vendor_id != 0x3434 || device.product_id != 0x0951 {
        return Err(SessionError::Usage(
            "macro writes require the V5 Max ISO (3434:0951) on a USB cable; receiver and \
             unknown-model writes are refused"
                .to_owned(),
        ));
    }
    let via_error = |error| SessionError::via(&error, false);
    let macro_count = via.macro_count().map_err(via_error)?;
    let buffer_size = via.macro_buffer_size().map_err(via_error)?;
    if expected.is_some_and(|backup| {
        backup.macro_count != macro_count || backup.buffer_size != buffer_size
    }) {
        return Err(SessionError::Usage(format!(
            "the keyboard's macro count ({macro_count}) and buffer size ({buffer_size}) do not \
             match the backup"
        )));
    }
    let backup = MacroBackup {
        format_version: BACKUP_VERSION,
        device: BackupDevice {
            vendor_id: device.vendor_id,
            product_id: device.product_id,
        },
        macro_count,
        buffer_size,
        buffer: via.macro_buffer(buffer_size).map_err(via_error)?,
    };
    backup.validate()?;
    Ok(backup)
}

fn verify(
    via: &mut Via<Session>,
    device: &KeychronDevice,
    expected: &MacroBackup,
) -> Result<(), SessionError> {
    if capture(via, device, Some(expected))? != *expected {
        return Err(SessionError::Usage(
            "macro buffer verification did not match the expected state".to_owned(),
        ));
    }
    Ok(())
}

fn edit_failure(error: SessionError, has_backup: bool, path: &Path) -> SessionError {
    if !has_backup {
        return error;
    }
    SessionError::Usage(format!(
        "{error}; macro state may have changed, and an unfinished write leaves every macro \
         disabled; pre-change macro backup retained at {}; undo with \
         `openkeychron macro restore {} --backup <NEW_FILE>`",
        path.display(),
        path.display()
    ))
}

fn print_success(
    operation: &'static str,
    summary: &str,
    backup: &Path,
    json: bool,
) -> Result<(), SessionError> {
    if json {
        return crate::print_json(&EditOutput { operation, backup });
    }
    println!("{summary}");
    println!("Pre-change backup: {}", backup.display());
    println!(
        "Undo with: openkeychron macro restore {} --backup <NEW_FILE>",
        backup.display()
    );
    Ok(())
}
