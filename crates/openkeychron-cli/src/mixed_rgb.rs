//! `openkeychron rgb mixed`: Keychron's mixed RGB, where each LED belongs to a region and each
//! region plays its own effect list (`crates/openkeychron-keychron/src/mixed.rs` has the firmware
//! behaviour). Every write saves the whole mixed state first, so `restore` undoes any of them.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use clap::Subcommand;
use openkeychron_guard::keymap::BackupDevice;
use openkeychron_guard::rgb::{MixedReader, write_mixed};
use openkeychron_guard::{
    GuardedWriteError, WriteVerified, require_write_verified, write_verified_model,
    write_verified_models_text,
};
use openkeychron_hid::KeychronDevice;
use openkeychron_keychron::{
    FirmwareVersion, Keychron, KeychronError, MatrixSize, MixedEffect, MixedRgb,
};
use openkeychron_via::V5_MAX_FR_ISO;
use serde::{Deserialize, Serialize};

use crate::backup_file;
use crate::rgb::{KeyPosition, resolve_key};
use crate::session::{Session, SessionError};

const BACKUP_VERSION: u32 = 2;
/// Backups from before #112, which do not record the firmware build.
const PRE_FIRMWARE_BACKUP_VERSION: u32 = 1;
const WRITES: &str = "mixed RGB writes";
const SHOWN_WHEN: &str = "Mixed RGB shows while the lighting effect is Keychron's mixed RGB \
                          effect, whose number depends on the firmware build.";

#[derive(Debug, Subcommand)]
pub enum MixedAction {
    /// Print each region's keys and effect list. Only reads.
    Get,
    /// Move keys to a region. Saves the whole mixed RGB state to --backup first, reads every
    /// change back, and stores it on the keyboard only if it matches.
    Region {
        /// New file for the previous mixed RGB state, saved before writing (JSON).
        #[arg(long, value_name = "FILE", conflicts_with = "record")]
        backup: PathBuf,

        /// Region number, from 0, as `rgb mixed get` lists them.
        region: u8,
        /// French legends as `openkeychron keymap show` prints them (`A`, `Esc`, `Num 7`), or
        /// matrix positions as ROW,COLUMN (`4,0` for the left Shift).
        #[arg(required = true)]
        keys: Vec<String>,
    },
    /// Set one slot of a region's effect list. Saves the whole mixed RGB state to --backup
    /// first, reads every change back, and stores it on the keyboard only if it matches.
    Effect {
        /// New file for the previous mixed RGB state, saved before writing (JSON).
        #[arg(long, value_name = "FILE", conflicts_with = "record")]
        backup: PathBuf,

        /// Region number, from 0.
        region: u8,
        /// Slot number in the region's effect list, from 0.
        slot: u8,
        /// RGB matrix effect number; 0 empties the slot. Numbers depend on the firmware build.
        effect: u8,
        /// 0–255.
        hue: u8,
        /// 0–255.
        saturation: u8,
        /// 0–255.
        speed: u8,
        /// How long the effect plays before the region moves to its next effect.
        duration_ms: u32,
    },
    /// Restore a mixed RGB backup, saving a separate pre-restore backup first.
    Restore {
        /// New, separate file for the mixed RGB state before the restore (JSON).
        #[arg(long, value_name = "FILE", conflicts_with = "record")]
        backup: PathBuf,

        /// Restore even if the backup was taken on another firmware build, or does not record one
        /// (format version 1). Effect numbers depend on the build, so the effects can differ. A
        /// keyboard that does not report its build is still refused.
        #[arg(long)]
        allow_other_firmware: bool,

        /// Mixed RGB backup to restore.
        from: PathBuf,
    },
}

/// One slot of a region's effect list, as backups and `--json` hold it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EffectSlot {
    effect: u8,
    hue: u8,
    saturation: u8,
    speed: u8,
    duration_ms: u32,
}

impl From<MixedEffect> for EffectSlot {
    fn from(effect: MixedEffect) -> Self {
        Self {
            effect: effect.effect,
            hue: effect.hue,
            saturation: effect.saturation,
            speed: effect.speed,
            duration_ms: effect.duration_ms,
        }
    }
}

impl From<EffectSlot> for MixedEffect {
    fn from(slot: EffectSlot) -> Self {
        Self {
            effect: slot.effect,
            hue: slot.hue,
            saturation: slot.saturation,
            speed: slot.speed,
            duration_ms: slot.duration_ms,
        }
    }
}

/// The Keychron firmware build (`0xA1`) whose effect numbers a backup's effect lists use.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FirmwareBuild {
    version: String,
    build_date: String,
}

impl From<FirmwareVersion> for FirmwareBuild {
    fn from(firmware: FirmwareVersion) -> Self {
        Self {
            version: firmware.version,
            build_date: firmware.build_date,
        }
    }
}

impl std::fmt::Display for FirmwareBuild {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} built {}", self.version, self.build_date)
    }
}

/// The whole mixed RGB state: every LED's region and every region's effect list.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MixedBackup {
    format_version: u32,
    device: BackupDevice,
    /// `None` only in a format version 1 backup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    firmware: Option<FirmwareBuild>,
    led_count: u8,
    effects_per_region: u8,
    /// One region number per LED.
    regions: Vec<u8>,
    /// One effect list per region.
    effects: Vec<Vec<EffectSlot>>,
}

impl MixedBackup {
    /// Checks the backup holds a consistent state for a model verified for writes, before any
    /// device is opened.
    fn validate(&self, path: &Path) -> Result<(), SessionError> {
        let invalid = |reason: String| {
            SessionError::Usage(format!("mixed RGB backup {}: {reason}", path.display()))
        };
        match (self.format_version, &self.firmware) {
            (BACKUP_VERSION, Some(_)) | (PRE_FIRMWARE_BACKUP_VERSION, None) => {}
            (BACKUP_VERSION, None) => {
                return Err(invalid(format!(
                    "format version {BACKUP_VERSION} has no firmware build"
                )));
            }
            (PRE_FIRMWARE_BACKUP_VERSION, Some(_)) => {
                return Err(invalid(format!(
                    "format version {PRE_FIRMWARE_BACKUP_VERSION} cannot hold a firmware build"
                )));
            }
            (other, _) => {
                return Err(invalid(format!(
                    "format version {other} is not {PRE_FIRMWARE_BACKUP_VERSION} or {BACKUP_VERSION}"
                )));
            }
        }
        self.device
            .write_verified_model()
            .map_err(|error| invalid(error.to_string()))?;
        if self.regions.len() != usize::from(self.led_count) {
            return Err(invalid(format!(
                "has {} regions for {} LEDs",
                self.regions.len(),
                self.led_count
            )));
        }
        if self.effects.is_empty() || self.effects.len() > usize::from(u8::MAX) {
            return Err(invalid(format!(
                "has {} effect lists; expected 1 to 255",
                self.effects.len()
            )));
        }
        if let Some(list) = self
            .effects
            .iter()
            .find(|list| list.len() != usize::from(self.effects_per_region))
        {
            return Err(invalid(format!(
                "has an effect list of {} slots; expected {}",
                list.len(),
                self.effects_per_region
            )));
        }
        if let Some(region) = self
            .regions
            .iter()
            .find(|&&region| usize::from(region) >= self.effects.len())
        {
            return Err(invalid(format!(
                "puts an LED in region {region}, past its {} regions",
                self.effects.len()
            )));
        }
        Ok(())
    }

    /// Checks the backup was taken from a keyboard laid out like the one `reader` talks to.
    fn require_layout_of(
        &self,
        path: &Path,
        reader: &MixedReader<'_, '_, Session>,
    ) -> Result<(), SessionError> {
        let mismatch = |what: String| {
            SessionError::Usage(format!(
                "mixed RGB backup {} {what}; nothing was written",
                path.display()
            ))
        };
        if self.led_count != reader.led_count() {
            return Err(mismatch(format!(
                "holds {} LEDs; this keyboard has {}",
                self.led_count,
                reader.led_count()
            )));
        }
        if self.effects.len() != usize::from(reader.region_count()) {
            return Err(mismatch(format!(
                "holds {} regions; this keyboard has {}",
                self.effects.len(),
                reader.region_count()
            )));
        }
        if self.effects_per_region != reader.effects_per_region() {
            return Err(mismatch(format!(
                "holds {} effect slots per region; this keyboard has {}",
                self.effects_per_region,
                reader.effects_per_region()
            )));
        }
        Ok(())
    }

    /// Checks the backup was taken on the firmware build the keyboard runs, unless
    /// `allow_other_firmware`: the same effect number can be another effect on another build.
    fn require_firmware(
        &self,
        path: &Path,
        running: &FirmwareBuild,
        allow_other_firmware: bool,
    ) -> Result<(), SessionError> {
        if allow_other_firmware || self.firmware.as_ref() == Some(running) {
            return Ok(());
        }
        let taken_on = match &self.firmware {
            Some(build) => format!("was taken on Keychron firmware {build}"),
            None => format!(
                "does not record the firmware build its effect numbers belong to (format \
                 version {})",
                self.format_version
            ),
        };
        Err(SessionError::Usage(format!(
            "mixed RGB backup {} {taken_on}; this keyboard runs {running}. Effect numbers depend \
             on the build, so the restored regions could play other effects; nothing was \
             written; pass --allow-other-firmware to restore it anyway",
            path.display()
        )))
    }
}

#[derive(Debug, Serialize)]
struct MixedOutput {
    led_count: u8,
    effects_per_region: u8,
    regions: Vec<RegionOutput>,
}

#[derive(Debug, Serialize)]
struct RegionOutput {
    region: u8,
    keys: Vec<KeyOutput>,
    leds: Vec<u8>,
    effects: Vec<EffectSlot>,
}

#[derive(Debug, Serialize)]
struct KeyOutput {
    key: &'static str,
    row: u8,
    column: u8,
    led: u8,
}

pub fn run(
    action: &MixedAction,
    fixture: Option<&Path>,
    record: Option<&Path>,
    device: Option<&Path>,
    json: bool,
) -> Result<(), SessionError> {
    if record.is_some() && !matches!(action, MixedAction::Get) {
        return Err(SessionError::Usage(
            "--record is not supported for rgb mixed region/effect/restore; it must not \
             overwrite a protected backup"
                .to_owned(),
        ));
    }
    match action {
        MixedAction::Get => {
            let session = Session::start(fixture, record, device)?;
            require_built_in_layout(session.device())?;
            get(session, json)
        }
        MixedAction::Region {
            backup,
            region,
            keys,
        } => {
            // Resolved before opening the device, so a mistyped key never reaches the keyboard.
            let keys = keys
                .iter()
                .map(|key| resolve_key(key))
                .collect::<Result<Vec<_>, _>>()?;
            let session = Session::start(fixture, record, device)?;
            let verified = require_write_verified(WRITES, session.device())?;
            set_region(session, &verified, backup, *region, &keys)
        }
        MixedAction::Effect {
            backup,
            region,
            slot,
            effect,
            hue,
            saturation,
            speed,
            duration_ms,
        } => {
            let session = Session::start(fixture, record, device)?;
            let verified = require_write_verified(WRITES, session.device())?;
            let wanted = EffectSlot {
                effect: *effect,
                hue: *hue,
                saturation: *saturation,
                speed: *speed,
                duration_ms: *duration_ms,
            };
            set_effect(session, &verified, backup, *region, *slot, wanted)
        }
        MixedAction::Restore {
            backup,
            allow_other_firmware,
            from,
        } => {
            let restored: MixedBackup = backup_file::load(from, "mixed RGB")?;
            restored.validate(from)?;
            let session = Session::start(fixture, record, device)?;
            let verified = require_write_verified(WRITES, session.device())?;
            restore(
                session,
                &verified,
                backup,
                from,
                restored,
                *allow_other_firmware,
            )
        }
    }
}

/// Key names come from the built-in V5 Max ISO table, so `get` refuses any other device, and a
/// receiver, which does not identify the keyboard behind it.
fn require_built_in_layout(device: &KeychronDevice) -> Result<(), SessionError> {
    if !device.is_receiver && write_verified_model(device.vendor_id, device.product_id).is_some() {
        return Ok(());
    }
    let connection = if device.is_receiver { "receiver " } else { "" };
    Err(SessionError::Usage(format!(
        "`rgb mixed get` names keys from the built-in layout of {models}, so it needs that \
         keyboard on a USB cable (got {connection}{vendor_id:04x}:{product_id:04x})",
        models = write_verified_models_text(),
        vendor_id = device.vendor_id,
        product_id = device.product_id,
    )))
}

fn get(session: Session, json: bool) -> Result<(), SessionError> {
    let mut keychron = Keychron::new(session);
    let result = keychron
        .mixed_rgb()
        .map_err(SessionError::from)
        .and_then(|mut rgb| read_output(&mut rgb));
    let output = keychron.into_transport().complete(result)?;
    if json {
        return crate::print_json(&output);
    }
    print_text(&output);
    Ok(())
}

fn read_output(rgb: &mut MixedRgb<'_, Session>) -> Result<MixedOutput, SessionError> {
    let regions = rgb.regions()?;
    let mut effect_lists = Vec::with_capacity(usize::from(rgb.region_count()));
    for region in 0..rgb.region_count() {
        effect_lists.push(rgb.effects(region)?);
    }
    let mut leds_by_row = Vec::with_capacity(usize::from(V5_MAX_FR_ISO.rows));
    for row in 0..V5_MAX_FR_ISO.rows {
        leds_by_row.push(rgb.leds_in_row(v5_max_matrix(), row)?);
    }
    let keys = V5_MAX_FR_ISO.keys.iter().filter_map(|key| {
        let [row, column] = key.matrix;
        let led = leds_by_row[usize::from(row)][usize::from(column)]?;
        Some(KeyOutput {
            key: key.legends.base,
            row,
            column,
            led,
        })
    });
    let mut output = MixedOutput {
        led_count: rgb.led_count(),
        effects_per_region: rgb.effects_per_region(),
        regions: (0..rgb.region_count())
            .zip(effect_lists)
            .map(|(region, effects)| RegionOutput {
                region,
                keys: Vec::new(),
                leds: Vec::new(),
                effects: effects.into_iter().map(EffectSlot::from).collect(),
            })
            .collect(),
    };
    for (led, region) in (0..rgb.led_count()).zip(&regions) {
        output.regions[usize::from(*region)].leds.push(led);
    }
    for key in keys {
        let region = regions[usize::from(key.led)];
        output.regions[usize::from(region)].keys.push(key);
    }
    Ok(output)
}

fn print_text(output: &MixedOutput) {
    for region in &output.regions {
        let keys = region
            .keys
            .iter()
            .map(|key| key.key)
            .collect::<Vec<_>>()
            .join(", ");
        println!("Region {}: {keys}", region.region);
        let keyless = region
            .leds
            .iter()
            .filter(|led| !region.keys.iter().any(|key| key.led == **led))
            .map(u8::to_string)
            .collect::<Vec<_>>();
        if !keyless.is_empty() {
            println!("  LEDs without a key: {}", keyless.join(", "));
        }
        for (slot, effect) in region.effects.iter().enumerate() {
            println!("  Slot {slot}: {}", describe(*effect));
        }
    }
    println!("{SHOWN_WHEN}");
}

fn describe(slot: EffectSlot) -> String {
    if slot.effect == 0 {
        return "empty".to_owned();
    }
    format!(
        "effect {}, hue {}, saturation {}, speed {}, {} ms",
        slot.effect, slot.hue, slot.saturation, slot.speed, slot.duration_ms
    )
}

fn set_region(
    session: Session,
    verified: &WriteVerified,
    backup: &Path,
    region: u8,
    keys: &[KeyPosition],
) -> Result<(), SessionError> {
    let device = backup_device(session.device());
    write_mixed_state(
        session,
        verified,
        backup,
        |reader| {
            if region >= reader.region_count() {
                return Err(missing_region(region, reader.region_count()));
            }
            let firmware = read_firmware(reader)?;
            let previous = capture(reader, device, firmware)?;
            let mut wanted = previous.clone();
            for led in leds_under(reader, keys)? {
                wanted.regions[usize::from(led)] = region;
            }
            Ok((previous, wanted))
        },
        |rgb, previous, wanted| {
            apply(rgb, &wanted, previous)?;
            let names = keys.iter().map(|key| key.legend).collect::<Vec<_>>();
            println!(
                "Keys {} moved to mixed RGB region {region}; the whole state reads back as written \
                 and is saved.",
                names.join(", ")
            );
            print_saved(backup, "Previous mixed RGB state");
            Ok(())
        },
    )
}

fn set_effect(
    session: Session,
    verified: &WriteVerified,
    backup: &Path,
    region: u8,
    slot: u8,
    effect: EffectSlot,
) -> Result<(), SessionError> {
    let device = backup_device(session.device());
    write_mixed_state(
        session,
        verified,
        backup,
        |reader| {
            if region >= reader.region_count() {
                return Err(missing_region(region, reader.region_count()));
            }
            if slot >= reader.effects_per_region() {
                return Err(SessionError::Usage(format!(
                    "effect slot {slot} does not exist; each region has {}",
                    reader.effects_per_region()
                )));
            }
            let firmware = read_firmware(reader)?;
            let previous = capture(reader, device, firmware)?;
            let mut wanted = previous.clone();
            wanted.effects[usize::from(region)][usize::from(slot)] = effect;
            Ok((previous, wanted))
        },
        |rgb, previous, wanted| {
            apply(rgb, &wanted, previous)?;
            if effect.effect == 0 {
                println!(
                    "Region {region}, slot {slot} emptied; the whole state reads back as written \
                     and is saved."
                );
            } else {
                println!(
                    "Region {region}, slot {slot} set to {}; the whole state reads back as \
                     written and is saved.",
                    describe(effect)
                );
            }
            print_saved(backup, "Previous mixed RGB state");
            Ok(())
        },
    )
}

fn restore(
    session: Session,
    verified: &WriteVerified,
    backup: &Path,
    from: &Path,
    restored: MixedBackup,
    allow_other_firmware: bool,
) -> Result<(), SessionError> {
    let device = backup_device(session.device());
    if restored.device != device {
        return Err(SessionError::Usage(format!(
            "mixed RGB backup {} is for {:04x}:{:04x}; this keyboard is {:04x}:{:04x}",
            from.display(),
            restored.device.vendor_id,
            restored.device.product_id,
            device.vendor_id,
            device.product_id
        )));
    }
    write_mixed_state(
        session,
        verified,
        backup,
        |reader| {
            restored.require_layout_of(from, reader)?;
            let firmware = read_firmware(reader)?;
            restored.require_firmware(from, &firmware, allow_other_firmware)?;
            Ok((capture(reader, device, firmware)?, restored))
        },
        |rgb, previous, restored| {
            apply(rgb, &restored, previous)?;
            println!(
                "Mixed RGB state from {} restored; it reads back as written and is saved.",
                from.display()
            );
            match &restored.firmware {
                Some(build) if restored.firmware != previous.firmware => println!(
                    "It was restored although it was taken on another firmware build ({build}): \
                     check each region's effects with `openkeychron rgb mixed get`."
                ),
                None => println!(
                    "It was restored although it does not record the firmware build it was taken \
                     on: check each region's effects with `openkeychron rgb mixed get`."
                ),
                Some(_) => {}
            }
            print_saved(backup, "Mixed RGB state before the restore");
            Ok(())
        },
    )
}

/// Runs a mixed RGB write on `session`, keeping its previous state in the new file `backup`, read
/// back before anything is written. The session is ended whatever the outcome.
fn write_mixed_state<Plan>(
    session: Session,
    verified: &WriteVerified,
    backup: &Path,
    capture: impl FnOnce(&mut MixedReader<'_, '_, Session>) -> Result<(MixedBackup, Plan), SessionError>,
    write: impl FnOnce(&mut MixedRgb<'_, Session>, &MixedBackup, Plan) -> Result<(), SessionError>,
) -> Result<(), SessionError> {
    let mut keychron = Keychron::new(session);
    let result = write_mixed(
        &mut keychron,
        verified,
        capture,
        |previous| backup_file::save_new_verified(backup, "mixed RGB", previous),
        write,
    )
    .map(drop)
    .map_err(GuardedWriteError::into_error);
    keychron.into_transport().complete(result)
}

fn backup_device(device: &KeychronDevice) -> BackupDevice {
    BackupDevice {
        vendor_id: device.vendor_id,
        product_id: device.product_id,
    }
}

fn missing_region(region: u8, region_count: u8) -> SessionError {
    SessionError::Usage(format!(
        "mixed RGB region {region} does not exist; this keyboard has {region_count}"
    ))
}

const fn v5_max_matrix() -> MatrixSize {
    MatrixSize {
        rows: V5_MAX_FR_ISO.rows,
        columns: V5_MAX_FR_ISO.columns,
    }
}

/// The firmware build a backup records. A write is refused when the keyboard does not report it,
/// because its backup could not say which effects its effect numbers mean.
fn read_firmware(reader: &mut MixedReader<'_, '_, Session>) -> Result<FirmwareBuild, SessionError> {
    reader
        .firmware_version()?
        .map(FirmwareBuild::from)
        .map_err(|reason| {
            SessionError::Usage(format!(
                "mixed RGB writes record the Keychron firmware build in their backup, and this \
                 keyboard does not report it ({}); nothing was written",
                crate::not_reported(reason)
            ))
        })
}

/// Every LED's region and every region's effect list, taken on `firmware`.
fn capture(
    reader: &mut MixedReader<'_, '_, Session>,
    device: BackupDevice,
    firmware: FirmwareBuild,
) -> Result<MixedBackup, SessionError> {
    let regions = reader.regions()?;
    let mut effects = Vec::with_capacity(usize::from(reader.region_count()));
    for region in 0..reader.region_count() {
        effects.push(
            reader
                .effects(region)?
                .into_iter()
                .map(EffectSlot::from)
                .collect(),
        );
    }
    Ok(MixedBackup {
        format_version: BACKUP_VERSION,
        device,
        firmware: Some(firmware),
        led_count: reader.led_count(),
        effects_per_region: reader.effects_per_region(),
        regions,
        effects,
    })
}

/// The LED under each key, one `A8 06` request per row.
fn leds_under(
    reader: &mut MixedReader<'_, '_, Session>,
    keys: &[KeyPosition],
) -> Result<Vec<u8>, SessionError> {
    let rows = keys.iter().map(|key| key.row).collect::<BTreeSet<_>>();
    let mut leds_by_row = Vec::with_capacity(rows.len());
    for row in rows {
        leds_by_row.push((row, reader.leds_in_row(v5_max_matrix(), row)?));
    }
    keys.iter()
        .map(|key| {
            leds_by_row
                .iter()
                .find(|(row, _)| *row == key.row)
                .and_then(|(_, leds)| leds.get(usize::from(key.column)).copied().flatten())
                .ok_or_else(|| {
                    SessionError::Usage(format!(
                        "key `{}` ({},{}) has no LED",
                        key.legend, key.row, key.column
                    ))
                })
        })
        .collect()
}

/// Sets `wanted`, then saves it. If any request fails, the save included, `previous` is set
/// again in full and nothing is saved: left in RAM, a partial or unsaved change would reach
/// EEPROM with the next Keychron RGB save, from this tool or any other.
fn apply(
    rgb: &mut MixedRgb<'_, Session>,
    wanted: &MixedBackup,
    previous: &MixedBackup,
) -> Result<(), SessionError> {
    let Err(error) = set_state(rgb, wanted).and_then(|()| rgb.save()) else {
        return Ok(());
    };
    Err(match set_state(rgb, previous) {
        Ok(()) => SessionError::Keychron(format!(
            "{error}; the previous mixed RGB state is set again and nothing was saved"
        )),
        Err(restore) => SessionError::Keychron(format!(
            "{error}; setting the previous mixed RGB state again also failed ({restore}); \
             nothing was saved, and unplugging the keyboard drops the change"
        )),
    })
}

fn set_state(rgb: &mut MixedRgb<'_, Session>, state: &MixedBackup) -> Result<(), KeychronError> {
    rgb.set_regions(0, &state.regions)?;
    for (region, slots) in (0..rgb.region_count()).zip(&state.effects) {
        let effects = slots
            .iter()
            .copied()
            .map(MixedEffect::from)
            .collect::<Vec<_>>();
        rgb.set_effects(region, 0, &effects)?;
    }
    Ok(())
}

fn print_saved(backup: &Path, what: &str) {
    println!("{SHOWN_WHEN}");
    println!(
        "{what} saved to {}. Restore with:\n  openkeychron rgb mixed restore --backup <NEW_FILE> {}",
        backup.display(),
        backup.display()
    );
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{EffectSlot, MixedBackup};

    type BackupChange = fn(&mut MixedBackup);

    fn fixture_backup() -> MixedBackup {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/synthetic/mixed-rgb-backup.json");
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    fn refusal(change: impl FnOnce(&mut MixedBackup)) -> String {
        let mut backup = fixture_backup();
        change(&mut backup);
        backup
            .validate(Path::new("backup.json"))
            .unwrap_err()
            .to_string()
    }

    #[test]
    fn accepts_the_fixture_backup() {
        fixture_backup().validate(Path::new("backup.json")).unwrap();
    }

    #[test]
    fn refuses_inconsistent_backups_before_opening_a_device() {
        let cases: [(BackupChange, &str); 8] = [
            (
                |backup| backup.format_version = 3,
                "format version 3 is not 1 or 2",
            ),
            (
                |backup| backup.firmware = None,
                "format version 2 has no firmware build",
            ),
            (
                |backup| backup.format_version = 1,
                "format version 1 cannot hold a firmware build",
            ),
            (
                |backup| backup.device.product_id = 0x0952,
                "device 3434:0952 is not a model verified for writes",
            ),
            (|backup| backup.led_count = 97, "has 98 regions for 97 LEDs"),
            (
                |backup| backup.effects.clear(),
                "has 0 effect lists; expected 1 to 255",
            ),
            (
                |backup| {
                    backup.effects[1].pop();
                },
                "has an effect list of 4 slots; expected 5",
            ),
            (
                |backup| backup.regions[0] = 2,
                "puts an LED in region 2, past its 2 regions",
            ),
        ];
        for (change, expected) in cases {
            let message = refusal(change);
            assert!(message.contains(expected), "{message}");
        }
    }

    #[test]
    fn accepts_a_format_1_backup_without_a_firmware_build() {
        let mut backup = fixture_backup();
        backup.format_version = 1;
        backup.firmware = None;
        backup.validate(Path::new("backup.json")).unwrap();
    }

    #[test]
    fn describes_an_empty_slot_as_empty() {
        let empty = EffectSlot {
            effect: 0,
            hue: 10,
            saturation: 20,
            speed: 30,
            duration_ms: 40,
        };
        assert_eq!(super::describe(empty), "empty");
    }
}
