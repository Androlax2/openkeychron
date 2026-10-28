//! `openkeychron settings`: debounce and Snap Click (Keychron commands, `docs/protocol/keychron.md`).

use std::path::{Path, PathBuf};

use clap::{Subcommand, ValueEnum};
use openkeychron_keychron::{Debounce, DebounceAlgorithm, Keychron, SnapClickMode, SnapClickPair};
use serde::Serialize;

use crate::session::{Session, SessionError};

#[derive(Debug, Subcommand)]
pub enum SettingsAction {
    /// Print debounce and the active Snap Click pairs. Only reads.
    Get,
    /// Change one setting. Saves the previous value to --backup first and reads the new one back.
    Set {
        /// Where to save the previous value before writing (JSON).
        #[arg(long, value_name = "FILE")]
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
struct SettingsOutput {
    debounce: DebounceOutput,
    snap_click_pairs: Vec<PairOutput>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum Backup {
    Debounce(DebounceOutput),
    SnapClick(PairOutput),
}

pub fn run(
    action: &SettingsAction,
    fixture: Option<&Path>,
    record: Option<&Path>,
    json: bool,
) -> Result<(), SessionError> {
    let session = Session::start(fixture, record)?;
    if matches!(action, SettingsAction::Set { .. }) && session.device().is_receiver {
        return Err(SessionError::Usage(
            "settings are only written over a USB cable: writes through a receiver are not \
             verified yet (see docs/protocol/keychron-link.md)"
                .to_owned(),
        ));
    }
    let mut keychron = Keychron::new(session);
    let result = match action {
        SettingsAction::Get => get(&mut keychron, json),
        SettingsAction::Set { backup, setting } => set(&mut keychron, backup, setting),
    };
    result?;
    keychron.into_transport().finish()
}

fn get(keychron: &mut Keychron<Session>, json: bool) -> Result<(), SessionError> {
    let debounce = keychron.debounce()?;
    let pairs = keychron.snap_click_pairs()?;
    let output = SettingsOutput {
        debounce: debounce_output(debounce),
        snap_click_pairs: pairs
            .iter()
            .zip(0u8..)
            .filter(|(pair, _)| pair.mode != SnapClickMode::Off)
            .map(|(pair, index)| pair_output(index, *pair))
            .collect(),
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
    Ok(())
}

fn set(
    keychron: &mut Keychron<Session>,
    backup: &Path,
    setting: &Setting,
) -> Result<(), SessionError> {
    match *setting {
        Setting::Debounce { algorithm, time_ms } => {
            let previous = debounce_output(keychron.debounce()?);
            let restore = format!(
                "openkeychron settings set --backup <FILE> debounce {} {}",
                name(&previous.algorithm),
                previous.time_ms
            );
            write_backup(backup, &Backup::Debounce(previous))?;
            keychron.set_debounce(Debounce {
                algorithm: library_algorithm(algorithm),
                time_ms,
            })?;
            println!("Debounce set to {time_ms} ms, {}.", name(&algorithm));
            println!(
                "Previous value saved to {}. Restore with:\n  {restore}",
                backup.display()
            );
        }
        Setting::SnapClick {
            index,
            mode,
            first_key,
            second_key,
            save,
        } => {
            let pairs = keychron.snap_click_pairs()?;
            let previous = pairs.get(usize::from(index)).copied().ok_or_else(|| {
                SessionError::Usage(format!(
                    "Snap Click pair {index} does not exist; this keyboard has {}",
                    pairs.len()
                ))
            })?;
            let previous = pair_output(index, previous);
            let restore = format!(
                "openkeychron settings set --backup <FILE> snap-click {index} {} 0x{:02x} 0x{:02x}{}",
                name(&previous.mode),
                previous.keys[0],
                previous.keys[1],
                if save { " --save" } else { "" }
            );
            write_backup(backup, &Backup::SnapClick(previous))?;
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
            println!(
                "Previous value saved to {}. Restore with:\n  {restore}",
                backup.display()
            );
        }
    }
    Ok(())
}

fn write_backup(path: &Path, backup: &Backup) -> Result<(), SessionError> {
    let text = serde_json::to_string_pretty(backup)
        .map_err(|error| SessionError::Usage(format!("cannot encode the backup: {error}")))?;
    std::fs::write(path, text + "\n").map_err(|error| {
        SessionError::Usage(format!(
            "cannot write the backup to {}: {error}; nothing was changed",
            path.display()
        ))
    })
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
