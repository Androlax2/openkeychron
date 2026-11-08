//! `openkeychron settings`: debounce, Snap Click and wireless timers (Keychron commands,
//! `docs/protocol/keychron.md`).

use std::path::{Path, PathBuf};

use clap::{Subcommand, ValueEnum};
use openkeychron_guard::settings::{SettingsReader, write_settings};
use openkeychron_guard::{GuardedWriteError, WriteVerified, require_write_verified};
use openkeychron_keychron::{
    Debounce, DebounceAlgorithm, Keychron, SnapClickMode, SnapClickPair, WirelessTimers,
};
use serde::Serialize;

use crate::backup_file;
use crate::session::{Session, SessionError};

#[derive(Debug, Subcommand)]
pub enum SettingsAction {
    /// Print debounce, the active Snap Click pairs and the wireless timers. Only reads.
    Get,
    /// Change one setting. Saves the previous value to --backup first and reads the new one back.
    Set {
        /// New file to save the previous value to before writing (JSON). An existing file is refused.
        #[arg(long, value_name = "FILE", conflicts_with = "record")]
        backup: PathBuf,

        #[command(subcommand)]
        setting: Setting,
    },
}

#[derive(Debug, Subcommand)]
pub enum Setting {
    /// Debounce algorithm and time. The keyboard stores it at once.
    Debounce {
        algorithm: Algorithm,
        /// Milliseconds, 0–255 (0–127 for asymmetric-eager-defer-per-key).
        time_ms: u8,
    },
    /// One Snap Click pair. Kept until power-off unless --save is given.
    SnapClick {
        /// Pair number, from 0.
        index: u8,
        mode: Mode,
        /// First key, as a basic keycode (for example 0x04 for A).
        #[arg(value_parser = parse_keycode)]
        first_key: u8,
        /// Second key, as a basic keycode.
        #[arg(value_parser = parse_keycode)]
        second_key: u8,
        /// Also write all Snap Click pairs to the keyboard's permanent memory.
        #[arg(long)]
        save: bool,
    },
    /// Wireless backlight-off and sleep times, in seconds. The keyboard stores them at once.
    Wireless {
        /// Seconds without typing before the backlight turns off (at least 5).
        backlight_off_seconds: u16,
        /// Seconds without typing before the keyboard sleeps (at least 60).
        sleep_seconds: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Algorithm {
    SymmetricDeferGlobal,
    SymmetricDeferPerRow,
    SymmetricDeferPerKey,
    SymmetricEagerPerRow,
    SymmetricEagerPerKey,
    AsymmetricEagerDeferPerKey,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    Off,
    Regular,
    /// Keychron's "LKP", most likely (an inference, see docs/protocol/keychron.md).
    LastInput,
    FirstKey,
    SecondKey,
    Neutral,
}

#[derive(Debug, Serialize)]
struct DebounceOutput {
    algorithm: Algorithm,
    time_ms: u8,
}

#[derive(Debug, Serialize)]
struct PairOutput {
    index: u8,
    mode: Mode,
    keys: [u8; 2],
}

#[derive(Debug, Serialize)]
struct WirelessOutput {
    backlight_off_seconds: u16,
    sleep_seconds: u16,
}

#[derive(Debug, Serialize)]
struct SettingsOutput {
    debounce: DebounceOutput,
    snap_click_pairs: Vec<PairOutput>,
    wireless: WirelessOutput,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum Backup {
    Debounce(DebounceOutput),
    SnapClick(PairOutput),
    Wireless(WirelessOutput),
}

pub fn run(
    action: &SettingsAction,
    fixture: Option<&Path>,
    record: Option<&Path>,
    device: Option<&Path>,
    json: bool,
) -> Result<(), SessionError> {
    if record.is_some() && matches!(action, SettingsAction::Set { .. }) {
        return Err(SessionError::Usage(
            "--record is not supported for settings set; it must not overwrite a protected backup"
                .to_owned(),
        ));
    }
    let session = Session::start(fixture, record, device)?;
    let keychron = match action {
        SettingsAction::Get => {
            let mut keychron = Keychron::new(session);
            get(&mut keychron, json)?;
            keychron
        }
        SettingsAction::Set { backup, setting } => {
            let verified = require_write_verified("settings writes", session.device())?;
            let mut keychron = Keychron::new(session);
            set(&mut keychron, &verified, backup, setting)?;
            keychron
        }
    };
    keychron.into_transport().finish()
}

fn get(keychron: &mut Keychron<Session>, json: bool) -> Result<(), SessionError> {
    let debounce = keychron.debounce()?;
    let pairs = keychron.snap_click_pairs()?;
    let wireless = wireless_output(keychron.wireless_timers()?);
    let output = SettingsOutput {
        debounce: debounce_output(debounce),
        snap_click_pairs: pairs
            .iter()
            .zip(0u8..)
            .filter(|(pair, _)| pair.mode != SnapClickMode::Off)
            .map(|(pair, index)| pair_output(index, *pair))
            .collect(),
        wireless,
    };
    if json {
        return crate::print_json(&output);
    }
    println!(
        "Debounce:   {} ms, {}",
        output.debounce.time_ms,
        name(&output.debounce.algorithm)
    );
    if output.snap_click_pairs.is_empty() {
        println!("Snap Click: no active pairs");
    }
    for pair in &output.snap_click_pairs {
        println!(
            "Snap Click: pair {}: {} on keys 0x{:02x} and 0x{:02x}",
            pair.index,
            name(&pair.mode),
            pair.keys[0],
            pair.keys[1]
        );
    }
    println!(
        "Wireless:   backlight off after {} s, sleep after {} s",
        output.wireless.backlight_off_seconds, output.wireless.sleep_seconds
    );
    Ok(())
}

/// Saves the previous value to the new file `backup`, then writes. A failure returns before the
/// session is finished, as `settings set` always has.
fn set(
    keychron: &mut Keychron<Session>,
    verified: &WriteVerified,
    backup: &Path,
    setting: &Setting,
) -> Result<(), SessionError> {
    match *setting {
        Setting::Debounce { algorithm, time_ms } => write_setting(
            keychron,
            verified,
            backup,
            |reader| {
                let previous = debounce_output(reader.debounce()?);
                let restore = format!(
                    "openkeychron settings set --backup <NEW_FILE> debounce {} {}",
                    name(&previous.algorithm),
                    previous.time_ms
                );
                Ok((Backup::Debounce(previous), restore))
            },
            |keychron| {
                keychron.set_debounce(Debounce {
                    algorithm: library_algorithm(algorithm),
                    time_ms,
                })?;
                println!("Debounce set to {time_ms} ms, {}.", name(&algorithm));
                Ok(())
            },
        ),
        Setting::SnapClick {
            index,
            mode,
            first_key,
            second_key,
            save,
        } => write_setting(
            keychron,
            verified,
            backup,
            |reader| {
                let pairs = reader.snap_click_pairs()?;
                let previous = pairs.get(usize::from(index)).copied().ok_or_else(|| {
                    SessionError::Usage(format!(
                        "Snap Click pair {index} does not exist; this keyboard has {}",
                        pairs.len()
                    ))
                })?;
                let previous = pair_output(index, previous);
                let restore = format!(
                    "openkeychron settings set --backup <NEW_FILE> snap-click {index} {} 0x{:02x} 0x{:02x}{}",
                    name(&previous.mode),
                    previous.keys[0],
                    previous.keys[1],
                    if save { " --save" } else { "" }
                );
                Ok((Backup::SnapClick(previous), restore))
            },
            |keychron| {
                keychron.set_snap_click_pair(
                    index,
                    SnapClickPair {
                        mode: library_mode(mode),
                        keys: [first_key, second_key],
                    },
                )?;
                if save {
                    keychron.save_snap_click()?;
                }
                let kept = if save {
                    "saved"
                } else {
                    "kept until power-off (not saved)"
                };
                println!("Snap Click pair {index} set to {}, {kept}.", name(&mode));
                Ok(())
            },
        ),
        Setting::Wireless {
            backlight_off_seconds,
            sleep_seconds,
        } => write_setting(
            keychron,
            verified,
            backup,
            |reader| {
                let previous = wireless_output(reader.wireless_timers()?);
                let restore = format!(
                    "openkeychron settings set --backup <NEW_FILE> wireless {} {}",
                    previous.backlight_off_seconds, previous.sleep_seconds
                );
                Ok((Backup::Wireless(previous), restore))
            },
            |keychron| {
                keychron.set_wireless_timers(WirelessTimers {
                    backlight_off_seconds,
                    sleep_seconds,
                })?;
                println!(
                    "Wireless: backlight off after {backlight_off_seconds} s, sleep after \
                     {sleep_seconds} s."
                );
                Ok(())
            },
        ),
    }
}

/// Saves the previous value `capture` reads to the new file `backup`, then runs `write` and prints
/// the restore command `capture` returned with it.
fn write_setting(
    keychron: &mut Keychron<Session>,
    verified: &WriteVerified,
    backup: &Path,
    capture: impl FnOnce(&mut SettingsReader<'_, Session>) -> Result<(Backup, String), SessionError>,
    write: impl FnOnce(&mut Keychron<Session>) -> Result<(), SessionError>,
) -> Result<(), SessionError> {
    write_settings(
        keychron,
        verified,
        capture,
        |previous| backup_file::save_new(backup, "settings", previous),
        |keychron, _, restore| {
            write(keychron)?;
            println!(
                "Previous value saved to {}. Restore with:\n  {restore}",
                backup.display()
            );
            Ok(())
        },
    )
    .map(drop)
    .map_err(GuardedWriteError::into_error)
}

fn name(value: &impl ValueEnum) -> String {
    value
        .to_possible_value()
        .map_or_else(String::new, |value| value.get_name().to_owned())
}

fn debounce_output(debounce: Debounce) -> DebounceOutput {
    DebounceOutput {
        algorithm: cli_algorithm(debounce.algorithm),
        time_ms: debounce.time_ms,
    }
}

fn wireless_output(timers: WirelessTimers) -> WirelessOutput {
    WirelessOutput {
        backlight_off_seconds: timers.backlight_off_seconds,
        sleep_seconds: timers.sleep_seconds,
    }
}

fn pair_output(index: u8, pair: SnapClickPair) -> PairOutput {
    PairOutput {
        index,
        mode: cli_mode(pair.mode),
        keys: pair.keys,
    }
}

fn parse_keycode(text: &str) -> Result<u8, String> {
    let parsed = match text.strip_prefix("0x") {
        Some(hex) => u8::from_str_radix(hex, 16),
        None => text.parse(),
    };
    parsed.map_err(|_| format!("`{text}` is not a keycode from 0 to 255 (decimal or 0x hex)"))
}

const fn library_algorithm(algorithm: Algorithm) -> DebounceAlgorithm {
    match algorithm {
        Algorithm::SymmetricDeferGlobal => DebounceAlgorithm::SymmetricDeferGlobal,
        Algorithm::SymmetricDeferPerRow => DebounceAlgorithm::SymmetricDeferPerRow,
        Algorithm::SymmetricDeferPerKey => DebounceAlgorithm::SymmetricDeferPerKey,
        Algorithm::SymmetricEagerPerRow => DebounceAlgorithm::SymmetricEagerPerRow,
        Algorithm::SymmetricEagerPerKey => DebounceAlgorithm::SymmetricEagerPerKey,
        Algorithm::AsymmetricEagerDeferPerKey => DebounceAlgorithm::AsymmetricEagerDeferPerKey,
        Algorithm::None => DebounceAlgorithm::None,
    }
}

const fn cli_algorithm(algorithm: DebounceAlgorithm) -> Algorithm {
    match algorithm {
        DebounceAlgorithm::SymmetricDeferGlobal => Algorithm::SymmetricDeferGlobal,
        DebounceAlgorithm::SymmetricDeferPerRow => Algorithm::SymmetricDeferPerRow,
        DebounceAlgorithm::SymmetricDeferPerKey => Algorithm::SymmetricDeferPerKey,
        DebounceAlgorithm::SymmetricEagerPerRow => Algorithm::SymmetricEagerPerRow,
        DebounceAlgorithm::SymmetricEagerPerKey => Algorithm::SymmetricEagerPerKey,
        DebounceAlgorithm::AsymmetricEagerDeferPerKey => Algorithm::AsymmetricEagerDeferPerKey,
        DebounceAlgorithm::None => Algorithm::None,
    }
}

const fn library_mode(mode: Mode) -> SnapClickMode {
    match mode {
        Mode::Off => SnapClickMode::Off,
        Mode::Regular => SnapClickMode::Regular,
        Mode::LastInput => SnapClickMode::LastInput,
        Mode::FirstKey => SnapClickMode::FirstKey,
        Mode::SecondKey => SnapClickMode::SecondKey,
        Mode::Neutral => SnapClickMode::Neutral,
    }
}

const fn cli_mode(mode: SnapClickMode) -> Mode {
    match mode {
        SnapClickMode::Off => Mode::Off,
        SnapClickMode::Regular => Mode::Regular,
        SnapClickMode::LastInput => Mode::LastInput,
        SnapClickMode::FirstKey => Mode::FirstKey,
        SnapClickMode::SecondKey => Mode::SecondKey,
        SnapClickMode::Neutral => Mode::Neutral,
    }
}
