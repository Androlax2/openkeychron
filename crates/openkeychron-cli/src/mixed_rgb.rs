//! `openkeychron rgb mixed`: Keychron's mixed RGB, where each LED belongs to a region and each
//! region plays its own effect list (`crates/openkeychron-keychron/src/mixed.rs` has the firmware
//! behaviour). Every write saves the whole mixed state first, so `restore` undoes any of them.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use clap::Subcommand;
use openkeychron_hid::KeychronDevice;
use openkeychron_keychron::{Keychron, KeychronError, MatrixSize, MixedEffect, MixedRgb};
use openkeychron_via::V5_MAX_FR_ISO;
use serde::{Deserialize, Serialize};

use crate::keymap_backup::BackupDevice;
use crate::rgb::{KeyPosition, resolve_key};
use crate::rgb_guarded_write::{self, MixedReader};
use crate::session::{Session, SessionError};
use crate::{backup_file, guarded_write};

const BACKUP_VERSION: u32 = 1;
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

/// The whole mixed RGB state: every LED's region and every region's effect list.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MixedBackup {
    format_version: u32,
    device: BackupDevice,
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
        if self.format_version != BACKUP_VERSION {
            return Err(invalid(format!(
                "format version {} is not {BACKUP_VERSION}",
                self.format_version
            )));
        }
        self.device.write_verified_model().map_err(invalid)?;
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
        reader: &MixedReader<'_, '_>,
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
            guarded_write::require_write_verified(WRITES, session.device())?;
            set_region(session, backup, *region, &keys)
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
            guarded_write::require_write_verified(WRITES, session.device())?;
            let wanted = EffectSlot {
                effect: *effect,
                hue: *hue,
                saturation: *saturation,
                speed: *speed,
                duration_ms: *duration_ms,
            };
            set_effect(session, backup, *region, *slot, wanted)
        }
        MixedAction::Restore { backup, from } => {
            let restored: MixedBackup = backup_file::load(from, "mixed RGB")?;
            restored.validate(from)?;
            let session = Session::start(fixture, record, device)?;
            guarded_write::require_write_verified(WRITES, session.device())?;
            restore(session, backup, from, restored)
        }
    }
}

/// Key names come from the built-in V5 Max ISO table, so `get` refuses any other device, and a
/// receiver, which does not identify the keyboard behind it.
fn require_built_in_layout(device: &KeychronDevice) -> Result<(), SessionError> {
    if !device.is_receiver
        && guarded_write::write_verified_model(device.vendor_id, device.product_id).is_some()
    {
        return Ok(());
    }
    let connection = if device.is_receiver { "receiver " } else { "" };
    Err(SessionError::Usage(format!(
        "`rgb mixed get` names keys from the built-in layout of {models}, so it needs that \
         keyboard on a USB cable (got {connection}{vendor_id:04x}:{product_id:04x})",
        models = guarded_write::write_verified_models_text(),
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
    backup: &Path,
    region: u8,
    keys: &[KeyPosition],
) -> Result<(), SessionError> {
    let device = backup_device(session.device());
    rgb_guarded_write::write_mixed(
        session,
        backup,
        |reader| {
            if region >= reader.region_count() {
                return Err(missing_region(region, reader.region_count()));
            }
            let previous = capture(reader, device)?;
            let mut wanted = previous.clone();
            for led in leds_under(reader, keys)? {
                wanted.regions[usize::from(led)] = region;
            }
            Ok((previous, wanted))
        },
        |rgb, previous, wanted| {
            apply(rgb, &wanted, &previous)?;
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
    backup: &Path,
    region: u8,
    slot: u8,
    effect: EffectSlot,
) -> Result<(), SessionError> {
    let device = backup_device(session.device());
    rgb_guarded_write::write_mixed(
        session,
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
            let previous = capture(reader, device)?;
            let mut wanted = previous.clone();
            wanted.effects[usize::from(region)][usize::from(slot)] = effect;
            Ok((previous, wanted))
        },
        |rgb, previous, wanted| {
            apply(rgb, &wanted, &previous)?;
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
    backup: &Path,
    from: &Path,
    restored: MixedBackup,
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
    rgb_guarded_write::write_mixed(
        session,
        backup,
        |reader| {
            restored.require_layout_of(from, reader)?;
            Ok((capture(reader, device)?, restored))
        },
        |rgb, previous, restored| {
            apply(rgb, &restored, &previous)?;
            println!(
                "Mixed RGB state from {} restored; it reads back as written and is saved.",
                from.display()
            );
            print_saved(backup, "Mixed RGB state before the restore");
            Ok(())
        },
    )
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

/// Every LED's region and every region's effect list.
fn capture(
    reader: &mut MixedReader<'_, '_>,
    device: BackupDevice,
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
        led_count: reader.led_count(),
        effects_per_region: reader.effects_per_region(),
        regions,
        effects,
    })
}

/// The LED under each key, one `A8 06` request per row.
fn leds_under(
    reader: &mut MixedReader<'_, '_>,
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

/// Sets `wanted`, then saves it. If any request fails, `previous` is set again in full and
/// nothing is saved: left in RAM, a partial change would reach EEPROM with the next Keychron RGB
/// save, from this tool or any other.
fn apply(
    rgb: &mut MixedRgb<'_, Session>,
    wanted: &MixedBackup,
    previous: &MixedBackup,
) -> Result<(), SessionError> {
    if let Err(error) = set_state(rgb, wanted) {
        return Err(match set_state(rgb, previous) {
            Ok(()) => SessionError::Keychron(format!(
                "{error}; the previous mixed RGB state is set again and nothing was saved"
            )),
            Err(restore) => SessionError::Keychron(format!(
                "{error}; setting the previous mixed RGB state again also failed ({restore}); \
                 nothing was saved, and unplugging the keyboard drops the change"
            )),
        });
    }
    rgb.save()?;
    Ok(())
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
