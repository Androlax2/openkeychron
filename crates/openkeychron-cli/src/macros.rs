//! Macro commands. Writes replace the whole macro buffer, after saving it to a new backup.

use std::path::{Path, PathBuf};

use clap::{Args, Subcommand, ValueEnum};
use openkeychron_hid::KeychronDevice;
use openkeychron_via::{
    FRENCH_AZERTY, MacroAction, TextLayout, TypedStep, Via, buffer_from_slots, decode_macro,
    encode_macro, slots,
};
use serde::Serialize;

use crate::guarded_write::{self, ViaReader, WriteKind};
use crate::keymap_backup::BackupDevice;
use crate::macro_backup::{BACKUP_VERSION, MacroBackup};
use crate::session::{Session, SessionError};
use crate::{DeviceOutput, device_output};

const MACRO_WRITE: WriteKind = WriteKind {
    command: "macro",
    writes: "macro writes",
    failure_risk: "an unfinished write leaves every macro disabled",
    backup_contents: "macro",
};

#[derive(Debug, Subcommand)]
pub enum MacroCommand {
    /// Print every macro slot as actions. Only reads.
    List(ListArgs),
    /// Replace one macro after saving the whole macro buffer. Writes over USB cable only.
    ///
    /// Actions run in order: text:TEXT, tap:KEY, down:KEY, up:KEY, delay:MS. KEY is an 8-bit
    /// basic HID keycode, decimal or 0x hex. Text needs --host-layout. No action empties the
    /// slot.
    Set(SetArgs),
    /// Restore a macro backup, saving a separate pre-restore backup first.
    Restore(RestoreArgs),
}

/// The keyboard layout of the computer the macro types into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize)]
#[serde(rename_all = "snake_case")]
enum HostLayout {
    /// Text stays text; the firmware types it as US-ANSI keycodes.
    Us,
    /// French AZERTY: text becomes the keystrokes that type it.
    Fr,
}

impl HostLayout {
    /// `None` for `Us`: the firmware already types text for a US host.
    fn text_layout(self) -> Option<&'static TextLayout> {
        match self {
            Self::Us => None,
            Self::Fr => Some(&FRENCH_AZERTY),
        }
    }
}

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Show keystrokes as the text they type on this host layout.
    #[arg(long, value_enum)]
    host_layout: Option<HostLayout>,
}

#[derive(Debug, Args)]
pub struct SetArgs {
    /// Zero-based macro slot (M0 is 0).
    slot: u8,
    /// Host layout that text actions are typed for. Required when there is a text action.
    #[arg(long, value_enum)]
    host_layout: Option<HostLayout>,
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
    host_layout: Option<HostLayout>,
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
    /// Keystrokes that type this text on the requested host layout.
    HostText(String),
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

impl From<TypedStep> for ActionOutput {
    fn from(step: TypedStep) -> Self {
        match step {
            TypedStep::Text(text) => Self::HostText(text),
            TypedStep::Action(action) => action.into(),
        }
    }
}

fn action_outputs(actions: Vec<MacroAction>, host_layout: Option<HostLayout>) -> Vec<ActionOutput> {
    match host_layout.and_then(HostLayout::text_layout) {
        Some(layout) => layout
            .typed_steps(&actions)
            .into_iter()
            .map(Into::into)
            .collect(),
        None => actions.into_iter().map(Into::into).collect(),
    }
}

/// Expands text actions into keystrokes for the host layout, or keeps them as firmware text
/// for a US host. Text without a host layout is refused: either choice types the wrong
/// characters on some host.
fn host_actions(
    actions: &[MacroAction],
    host_layout: Option<HostLayout>,
) -> Result<Vec<MacroAction>, SessionError> {
    let mut expanded = Vec::new();
    let mut held_keys = Vec::new();
    for action in actions {
        match action {
            MacroAction::Down(key) => held_keys.push(*key),
            MacroAction::Up(key) => held_keys.retain(|held| held != key),
            _ => {}
        }
        let MacroAction::Text(text) = action else {
            expanded.push(action.clone());
            continue;
        };
        let host_layout = host_layout.ok_or_else(|| {
            SessionError::Usage(
                "text actions need --host-layout: `fr` types them as French AZERTY keystrokes, \
                 `us` stores text the firmware types as US-ANSI keycodes"
                    .to_owned(),
            )
        })?;
        let Some(layout) = host_layout.text_layout() else {
            expanded.push(action.clone());
            continue;
        };
        // The keystrokes press and release their own modifiers, so a key held around them
        // would change or cancel what they type.
        if let Some(held) = held_keys.first() {
            return Err(SessionError::Usage(format!(
                "text:{text:?} comes while down:0x{held:02x} is still held, which changes what \
                 it types; release it with up:0x{held:02x} before the text"
            )));
        }
        expanded.extend(
            layout
                .actions_for_text(text)
                .map_err(|error| SessionError::Usage(error.to_string()))?,
        );
    }
    Ok(expanded)
}

pub fn run(
    command: &MacroCommand,
    fixture: Option<&Path>,
    record: Option<&Path>,
    device: Option<&Path>,
    json: bool,
) -> Result<(), SessionError> {
    if record.is_some() && !matches!(command, MacroCommand::List(_)) {
        return Err(SessionError::Usage(
            "--record is not supported for macro set/restore; it must not overwrite a protected backup".to_owned(),
        ));
    }
    match command {
        MacroCommand::List(args) => list(
            Session::start(fixture, record, device)?,
            args.host_layout,
            json,
        ),
        MacroCommand::Set(args) => set(args, fixture, device, json),
        MacroCommand::Restore(args) => restore(args, fixture, device, json),
    }
}

fn list(session: Session, host_layout: Option<HostLayout>, json: bool) -> Result<(), SessionError> {
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
                    Ok(actions) => (Some(action_outputs(actions, host_layout)), None),
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
            host_layout,
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
    let text_note = match output.host_layout.and_then(HostLayout::text_layout) {
        Some(layout) => format!(
            "text is what keystrokes type on a {} host; us-text types US-ANSI keycodes",
            layout.name
        ),
        None => "text types US-ANSI keycodes".to_owned(),
    };
    println!(
        "Macros: {} slots, {}-byte buffer. Keys are raw HID keycodes; {text_note}.",
        output.macro_count, output.buffer_size
    );
    let is_host_text_shown = output
        .host_layout
        .and_then(HostLayout::text_layout)
        .is_some();
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
                .map(|action| action_text(action, is_host_text_shown))
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

/// With host text shown, `text:` is host text, as `macro set --host-layout` takes it, and
/// firmware text is `us-text:`.
fn action_text(action: &ActionOutput, is_host_text_shown: bool) -> String {
    match action {
        ActionOutput::Text(text) if is_host_text_shown => format!("us-text:{text:?}"),
        ActionOutput::Text(text) | ActionOutput::HostText(text) => format!("text:{text:?}"),
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
    let actions = host_actions(&args.actions, args.host_layout)?;
    let encoded = encode_macro(&actions).map_err(|error| SessionError::Usage(error.to_string()))?;
    guarded_write::run(
        &MACRO_WRITE,
        fixture,
        device,
        &args.backup,
        |reader, identity| {
            let previous = capture(reader, identity, None)?;
            if !previous.macros_enabled() {
                return Err(SessionError::Usage(
                    "the macro buffer's last byte is not 0, so an earlier write did not finish \
                     and its content is unknown; run `openkeychron macro restore` with a backup \
                     first"
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
            Ok((previous, buffer))
        },
        |via, identity, previous, buffer| {
            via.write_macro_buffer(&buffer)
                .map_err(|error| SessionError::via(&error, false))?;
            verify(via, identity, &MacroBackup { buffer, ..previous })
        },
    )?;
    guarded_write::print_success(
        &MACRO_WRITE,
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
    guarded_write::run(
        &MACRO_WRITE,
        fixture,
        device,
        &args.backup,
        |reader, identity| Ok((capture(reader, identity, Some(&wanted))?, ())),
        |via, identity, _previous, ()| {
            via.write_macro_buffer(&wanted.buffer)
                .map_err(|error| SessionError::via(&error, false))?;
            verify(via, identity, &wanted)
        },
    )?;
    guarded_write::print_success(
        &MACRO_WRITE,
        "restore",
        "Macro buffer restored and verified.",
        &args.backup,
        json,
    )
}

/// Reads the whole macro buffer. With `expected`, the keyboard must report the same macro
/// count and buffer size, so a restore never writes a buffer laid out for other firmware.
fn capture(
    via: &mut ViaReader<'_>,
    device: &KeychronDevice,
    expected: Option<&MacroBackup>,
) -> Result<MacroBackup, SessionError> {
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
    if capture(&mut ViaReader::new(via), device, Some(expected))? != *expected {
        return Err(SessionError::Usage(
            "macro buffer verification did not match the expected state".to_owned(),
        ));
    }
    Ok(())
}
