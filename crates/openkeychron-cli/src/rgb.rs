//! `openkeychron rgb`: RGB matrix effect, speed, brightness and colour, through VIA's lighting
//! channel (`crates/openkeychron-via/src/rgb.rs` has the firmware behaviour).

use std::path::{Path, PathBuf};

use clap::Subcommand;
use openkeychron_via::{RgbMatrix, RgbValue, Via, brightness_to_store};
use serde::Serialize;

use crate::session::{Session, SessionError};

#[derive(Debug, Subcommand)]
pub enum RgbAction {
    /// Print brightness, effect, speed and colour. Only reads.
    Get,
    /// Change one value. Saves the previous values to --backup first, reads the new one back,
    /// and stores it on the keyboard only if it matches.
    Set {
        /// Where to save the previous values before writing (JSON).
        #[arg(long, value_name = "FILE")]
        backup: PathBuf,

        #[command(subcommand)]
        value: Value,
    },
}

#[derive(Debug, Subcommand)]
pub enum Value {
    /// 0–255. The keyboard stores a slightly lower value (255 reads back as 254).
    Brightness { value: u8 },
    /// Effect number; 0 turns lighting off. Numbers depend on the firmware build.
    Effect { value: u8 },
    /// 0–255.
    Speed { value: u8 },
    /// Hue and saturation, 0–255 each.
    Color { hue: u8, saturation: u8 },
}

#[derive(Debug, Serialize)]
struct RgbOutput {
    brightness: u8,
    effect: u8,
    speed: u8,
    hue: u8,
    saturation: u8,
}

pub fn run(
    action: &RgbAction,
    fixture: Option<&Path>,
    record: Option<&Path>,
    device: Option<&Path>,
    json: bool,
) -> Result<(), SessionError> {
    let session = Session::start(fixture, record, device)?;
    if matches!(action, RgbAction::Set { .. }) && session.device().is_receiver {
        return Err(SessionError::Usage(
            "lighting is only written over a USB cable: writes through a receiver are not \
             verified yet (see docs/protocol/keychron-link.md)"
                .to_owned(),
        ));
    }
    let through_receiver = session.device().is_receiver;
    let mut via = Via::new(session);
    let via_error = |error: openkeychron_via::ViaError| SessionError::via(&error, through_receiver);
    match action {
        RgbAction::Get => get(&mut via, json, via_error)?,
        RgbAction::Set { backup, value } => set(&mut via, backup, value, via_error)?,
    }
    via.into_transport().finish()
}

fn get(
    via: &mut Via<Session>,
    json: bool,
    via_error: impl Fn(openkeychron_via::ViaError) -> SessionError,
) -> Result<(), SessionError> {
    let rgb = output(via.rgb_matrix().map_err(via_error)?);
    if json {
        return crate::print_json(&rgb);
    }
    println!("Brightness: {}", rgb.brightness);
    if rgb.effect == 0 {
        println!("Effect:     off");
    } else {
        println!("Effect:     {}", rgb.effect);
    }
    println!("Speed:      {}", rgb.speed);
    println!("Colour:     hue {}, saturation {}", rgb.hue, rgb.saturation);
    Ok(())
}

fn set(
    via: &mut Via<Session>,
    backup: &Path,
    value: &Value,
    via_error: impl Fn(openkeychron_via::ViaError) -> SessionError,
) -> Result<(), SessionError> {
    // Reading first also proves the firmware has the lighting channel before anything changes.
    let previous = output(via.rgb_matrix().map_err(&via_error)?);
    let restore = restore_command(&previous, value);
    write_backup(backup, &previous)?;

    let wanted = match *value {
        Value::Brightness { value } => RgbValue::Brightness(value),
        Value::Effect { value } => RgbValue::Effect(value),
        Value::Speed { value } => RgbValue::Speed(value),
        Value::Color { hue, saturation } => RgbValue::Color { hue, saturation },
    };
    via.set_rgb_value(wanted).map_err(&via_error)?;

    println!(
        "Set {:?}; it reads back as {:?} and is saved.",
        wanted,
        wanted.expected_read_back()
    );
    println!(
        "Previous values saved to {}. Restore with:\n  {restore}",
        backup.display()
    );
    Ok(())
}

fn restore_command(previous: &RgbOutput, value: &Value) -> String {
    let prefix = "openkeychron rgb set --backup <FILE>";
    match value {
        Value::Brightness { .. } => match brightness_to_store(previous.brightness) {
            Some(send) => format!("{prefix} brightness {send}"),
            None => format!(
                "{prefix} brightness 255   (closest: the keyboard stored {}, which VIA cannot set)",
                previous.brightness
            ),
        },
        Value::Effect { .. } => format!("{prefix} effect {}", previous.effect),
        Value::Speed { .. } => format!("{prefix} speed {}", previous.speed),
        Value::Color { .. } => format!("{prefix} color {} {}", previous.hue, previous.saturation),
    }
}

fn write_backup(path: &Path, previous: &RgbOutput) -> Result<(), SessionError> {
    let text = serde_json::to_string_pretty(previous)
        .map_err(|error| SessionError::Usage(format!("cannot encode the backup: {error}")))?;
    std::fs::write(path, text + "\n").map_err(|error| {
        SessionError::Usage(format!(
            "cannot write the backup to {}: {error}; nothing was changed",
            path.display()
        ))
    })
}

fn output(rgb: RgbMatrix) -> RgbOutput {
    RgbOutput {
        brightness: rgb.brightness,
        effect: rgb.effect,
        speed: rgb.speed,
        hue: rgb.hue,
        saturation: rgb.saturation,
    }
}
