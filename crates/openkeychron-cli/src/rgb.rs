//! `openkeychron rgb`: RGB matrix effect, speed, brightness and colour, through VIA's lighting
//! channel (`crates/openkeychron-via/src/rgb.rs` has the firmware behaviour), and per-key colours
//! through Keychron's RGB commands (`crates/openkeychron-keychron/src/perkey.rs`). `rgb mixed`
//! is in `mixed_rgb.rs`.

use std::path::{Path, PathBuf};

use clap::Subcommand;
use openkeychron_guard::rgb::{PerKeyReader, set_key_color, write_lighting, write_per_key};
use openkeychron_guard::{GuardedWriteError, WriteVerified, require_write_verified};
use openkeychron_keychron::{Keychron, MatrixSize, PerKeyColor, PerKeyRgb};
use openkeychron_via::{RgbMatrix, RgbValue, V5_MAX_FR_ISO, Via, brightness_to_store};
use serde::Serialize;

use crate::backup_file;
use crate::mixed_rgb::{self, MixedAction};
use crate::session::{Session, SessionError};

#[derive(Debug, Subcommand)]
pub enum RgbAction {
    /// Print brightness, effect, speed and colour. Only reads.
    Get,
    /// Change one value. Saves the previous values to --backup first, reads the new one back,
    /// and stores it on the keyboard only if it matches.
    Set {
        /// New file for the previous values, saved before writing (JSON).
        #[arg(long, value_name = "FILE", conflicts_with = "record")]
        backup: PathBuf,

        #[command(subcommand)]
        value: Value,
    },
    /// Set one key's colour (per-key RGB, Keychron firmware 1.1.0 or later). Saves the previous
    /// colour to --backup first, reads the new one back, and stores it only if it matches.
    /// Shown only while the lighting effect is Keychron's per-key effect.
    Key {
        /// New file for the previous colour, saved before writing (JSON).
        #[arg(long, value_name = "FILE", conflicts_with = "record")]
        backup: PathBuf,

        /// A French legend as `openkeychron keymap show` prints it (`A`, `Esc`, `Num 7`), or a
        /// matrix position as ROW,COLUMN (`4,0` for the left Shift).
        key: String,
        /// 0–255.
        hue: u8,
        /// 0–255.
        saturation: u8,
    },
    /// Mixed RGB (Keychron firmware 1.1.0 or later): each key belongs to a region, and each
    /// region plays its own list of effects.
    Mixed {
        #[command(subcommand)]
        action: MixedAction,
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

/// A key of the V5 Max ISO layout, found by legend or matrix position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct KeyPosition {
    pub(crate) legend: &'static str,
    pub(crate) row: u8,
    pub(crate) column: u8,
}

#[derive(Debug, Serialize)]
struct KeyBackup {
    key: &'static str,
    row: u8,
    column: u8,
    led: u8,
    hue: u8,
    saturation: u8,
    value: u8,
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
    if record.is_some() && matches!(action, RgbAction::Set { .. } | RgbAction::Key { .. }) {
        return Err(SessionError::Usage(
            "--record is not supported for rgb set/key; it must not overwrite a protected backup"
                .to_owned(),
        ));
    }
    match action {
        RgbAction::Get => {
            let session = Session::start(fixture, record, device)?;
            let through_receiver = session.device().is_receiver;
            let mut via = Via::new(session);
            get(&mut via, json, via_error(through_receiver))?;
            via.into_transport().finish()
        }
        RgbAction::Set { backup, value } => {
            let session = Session::start(fixture, record, device)?;
            let verified = require_write_verified("RGB writes", session.device())?;
            set(session, &verified, backup, value, via_error(false))
        }
        RgbAction::Key {
            backup,
            key,
            hue,
            saturation,
        } => {
            // Resolved before opening the device, so a mistyped key never reaches the keyboard.
            let key = resolve_key(key)?;
            let session = Session::start(fixture, record, device)?;
            set_key(session, backup, key, *hue, *saturation)
        }
        RgbAction::Mixed { action } => mixed_rgb::run(action, fixture, record, device, json),
    }
}

fn via_error(through_receiver: bool) -> impl Fn(openkeychron_via::ViaError) -> SessionError {
    move |error| SessionError::via(&error, through_receiver)
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

/// A failure returns before the session is finished, as `rgb set` always has.
fn set(
    session: Session,
    verified: &WriteVerified,
    backup: &Path,
    value: &Value,
    via_error: impl Fn(openkeychron_via::ViaError) -> SessionError,
) -> Result<(), SessionError> {
    let mut via = Via::new(session);
    write_lighting(
        &mut via,
        verified,
        // Reading first also proves the firmware has the lighting channel before anything changes.
        |reader| Ok((output(reader.rgb_matrix().map_err(&via_error)?), ())),
        |previous| backup_file::save_new(backup, "RGB", previous),
        |via, previous, ()| {
            let restore = restore_command(previous, value);
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
        },
    )
    .map_err(GuardedWriteError::into_error)?;
    via.into_transport().finish()
}

fn set_key(
    session: Session,
    backup: &Path,
    key: KeyPosition,
    hue: u8,
    saturation: u8,
) -> Result<(), SessionError> {
    let verified = require_write_verified("per-key RGB writes", session.device())?;
    let mut keychron = Keychron::new(session);
    let result = write_per_key(
        &mut keychron,
        &verified,
        |reader| Ok((capture_key_color(reader, key)?, ())),
        |previous| backup_file::save_new(backup, "per-key RGB", previous),
        |rgb, previous, ()| write_key_color(rgb, backup, previous, hue, saturation),
    )
    .map(drop)
    .map_err(GuardedWriteError::into_error);
    keychron.into_transport().complete(result)
}

/// The LED under `key` and its colour.
fn capture_key_color(
    reader: &mut PerKeyReader<'_, '_, Session>,
    key: KeyPosition,
) -> Result<KeyBackup, SessionError> {
    let matrix = MatrixSize {
        rows: V5_MAX_FR_ISO.rows,
        columns: V5_MAX_FR_ISO.columns,
    };
    let led = reader
        .led_index(matrix, key.row, key.column)?
        .ok_or_else(|| {
            SessionError::Usage(format!(
                "key `{}` ({},{}) has no LED",
                key.legend, key.row, key.column
            ))
        })?;
    let previous = reader.color(led)?;
    Ok(KeyBackup {
        key: key.legend,
        row: key.row,
        column: key.column,
        led,
        hue: previous.hue,
        saturation: previous.saturation,
        value: previous.value,
    })
}

fn write_key_color(
    rgb: &mut PerKeyRgb<'_, Session>,
    backup: &Path,
    previous: &KeyBackup,
    hue: u8,
    saturation: u8,
) -> Result<(), SessionError> {
    let led = previous.led;
    let previous_color = PerKeyColor {
        hue: previous.hue,
        saturation: previous.saturation,
        value: previous.value,
    };
    set_key_color(rgb, led, previous_color, hue, saturation)?;

    println!(
        "Key `{}` ({},{}, LED {led}) set to hue {hue}, saturation {saturation}; it reads back \
         the same and is saved.",
        previous.key, previous.row, previous.column
    );
    println!(
        "It shows while the lighting effect is Keychron's per-key effect, whose number depends \
         on the firmware build."
    );
    println!(
        "Previous colour saved to {}. Restore with:\n  openkeychron rgb key --backup <NEW_FILE> \
         {},{} {} {}",
        backup.display(),
        previous.row,
        previous.column,
        previous.hue,
        previous.saturation
    );
    Ok(())
}

/// Finds a key by ROW,COLUMN or by its French legend, ignoring case.
pub(crate) fn resolve_key(name: &str) -> Result<KeyPosition, SessionError> {
    let layout = V5_MAX_FR_ISO;
    if let Some((row, column)) = name.split_once(',')
        && let (Ok(row), Ok(column)) = (row.trim().parse::<u8>(), column.trim().parse::<u8>())
    {
        return layout
            .key(row, column)
            .map(|key| KeyPosition {
                legend: key.legends.base,
                row,
                column,
            })
            .ok_or_else(|| {
                SessionError::Usage(format!(
                    "the V5 Max ISO has no key at matrix position {row},{column}"
                ))
            });
    }
    let wanted = name.to_lowercase();
    let matches: Vec<KeyPosition> = layout
        .keys
        .iter()
        .filter(|key| key.legends.base.to_lowercase() == wanted)
        .map(|key| KeyPosition {
            legend: key.legends.base,
            row: key.matrix[0],
            column: key.matrix[1],
        })
        .collect();
    match matches.as_slice() {
        [key] => Ok(*key),
        [] => Err(SessionError::Usage(format!(
            "no V5 Max ISO key has the legend `{name}`; use a legend from `openkeychron keymap \
             show` or ROW,COLUMN"
        ))),
        several => Err(SessionError::Usage(format!(
            "`{name}` names several keys ({}); give ROW,COLUMN",
            several
                .iter()
                .map(|key| format!("{},{}", key.row, key.column))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

fn restore_command(previous: &RgbOutput, value: &Value) -> String {
    let prefix = "openkeychron rgb set --backup <NEW_FILE>";
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

fn output(rgb: RgbMatrix) -> RgbOutput {
    RgbOutput {
        brightness: rgb.brightness,
        effect: rgb.effect,
        speed: rgb.speed,
        hue: rgb.hue,
        saturation: rgb.saturation,
    }
}

#[cfg(test)]
mod tests {
    use super::{KeyPosition, resolve_key};

    #[test]
    fn perkey_finds_a_key_by_its_legend_ignoring_case() {
        assert_eq!(
            resolve_key("num 7").ok(),
            Some(KeyPosition {
                legend: "Num 7",
                row: 2,
                column: 15,
            })
        );
    }

    #[test]
    fn perkey_finds_a_key_by_matrix_position() {
        assert_eq!(
            resolve_key("4,12").ok(),
            Some(KeyPosition {
                legend: "Shift",
                row: 4,
                column: 12,
            })
        );
    }

    #[test]
    fn perkey_reads_a_lone_comma_as_the_comma_key() {
        assert_eq!(
            resolve_key(",").ok(),
            Some(KeyPosition {
                legend: ",",
                row: 4,
                column: 8,
            })
        );
    }

    #[test]
    fn perkey_rejects_a_matrix_position_without_a_key() {
        assert_eq!(
            resolve_key("0,1").err().map(|error| error.to_string()),
            Some("the V5 Max ISO has no key at matrix position 0,1".to_owned())
        );
    }

    #[test]
    fn perkey_rejects_an_unknown_legend() {
        assert!(
            resolve_key("Menu")
                .err()
                .is_some_and(|error| error.to_string().contains("no V5 Max ISO key has"))
        );
    }
}
