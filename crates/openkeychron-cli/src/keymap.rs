//! Keymap commands. Physical French legends name positions, not live assignments.

use std::path::{Path, PathBuf};

use clap::Subcommand;
use openkeychron_via::{DeviceRegistry, Keycode, Via, ViaDefinition};
use serde::Serialize;

use crate::session::{Session, SessionError};
use crate::{DeviceOutput, device_output};

#[derive(Debug, Subcommand)]
pub enum KeymapAction {
    /// Print each layer as a matrix grid with QMK names and raw keycodes: French legends on the
    /// built-in V5 Max ISO, positions for a layout read with --layout. Only reads.
    Show {
        /// Show only this zero-based layer (all layers are still read).
        #[arg(long)]
        layer: Option<u8>,
        /// Read the physical layout from this VIA definition file, for a keyboard other
        /// than the built-in V5 Max ISO. Keychron publishes one per model.
        #[arg(long, value_name = "VIA_JSON")]
        layout: Option<PathBuf>,
    },
    /// Set one raw keycode after saving a complete matrix/encoder backup. Writes over USB cable only.
    Set(crate::keymap_edit::SetArgs),
    /// Restore matrix/encoder assignments, saving a separate pre-restore backup first.
    Restore(crate::keymap_edit::RestoreArgs),
    /// Reset every layer's matrix and encoder keys to firmware defaults, after saving a
    /// complete backup. Writes over USB cable only; undo with keymap restore.
    Reset(crate::keymap_edit::ResetArgs),
}

#[derive(Debug, Serialize)]
struct KeymapOutput {
    device: DeviceOutput,
    layer_count: u8,
    rows: u8,
    columns: u8,
    layers: Vec<LayerOutput>,
}

#[derive(Debug, Serialize)]
struct LayerOutput {
    index: u8,
    keys: Vec<KeyOutput>,
}

#[derive(Debug, Serialize)]
struct KeyOutput {
    row: u8,
    column: u8,
    legend: Option<String>,
    /// False for matrix slots no physical key uses.
    is_physical: bool,
    keycode: u16,
}

pub fn run(
    action: &KeymapAction,
    fixture: Option<&Path>,
    record: Option<&Path>,
    device: Option<&Path>,
    json: bool,
) -> Result<(), SessionError> {
    if record.is_some()
        && matches!(
            action,
            KeymapAction::Set(_) | KeymapAction::Restore(_) | KeymapAction::Reset(_)
        )
    {
        return Err(SessionError::Usage(
            "--record is not supported for keymap set/restore/reset; it must not overwrite a protected backup".to_owned()
        ));
    }
    match action {
        KeymapAction::Show { layer, layout } => {
            let definition = layout.as_deref().map(load_definition).transpose()?;
            let output = query(Session::start(fixture, record, device)?, *layer, definition)?;
            if json {
                return crate::print_json(&output);
            }
            print_grid(&output);
            Ok(())
        }
        KeymapAction::Set(args) => crate::keymap_edit::set(args, fixture, device, json),
        KeymapAction::Restore(args) => crate::keymap_edit::restore(args, fixture, device, json),
        KeymapAction::Reset(args) => crate::keymap_edit::reset(args, fixture, device, json),
    }
}

fn load_definition(path: &Path) -> Result<ViaDefinition, SessionError> {
    let text = std::fs::read_to_string(path).map_err(|error| {
        SessionError::Usage(format!(
            "cannot read layout file {}: {error}",
            path.display()
        ))
    })?;
    ViaDefinition::from_json(&text)
        .map_err(|error| SessionError::Usage(format!("layout file {}: {error}", path.display())))
}

fn query(
    session: Session,
    selected_layer: Option<u8>,
    definition: Option<ViaDefinition>,
) -> Result<KeymapOutput, SessionError> {
    let device = session.device().clone();
    let mut via = Via::new(session);
    let result = (|| {
        if device.is_receiver {
            return Err(SessionError::Usage(
                "keymap show needs the keyboard on a USB cable: a receiver does not identify \
                 the keyboard behind it"
                    .to_owned(),
            ));
        }
        let mut registry = DeviceRegistry::built_in();
        if let Some(definition) = definition {
            if (definition.vendor_id, definition.product_id)
                != (device.vendor_id, device.product_id)
            {
                return Err(SessionError::Usage(format!(
                    "the layout file is for {} ({:04x}:{:04x}), but the keyboard is \
                     {:04x}:{:04x}",
                    definition.name,
                    definition.vendor_id,
                    definition.product_id,
                    device.vendor_id,
                    device.product_id
                )));
            }
            registry.register(definition).map_err(|error| {
                SessionError::Usage(format!("{error}; run keymap show without --layout"))
            })?;
        }
        let layout = registry
            .layout(device.vendor_id, device.product_id)
            .ok_or_else(|| {
                SessionError::Usage(format!(
                    "keymap show has no layout for {:04x}:{:04x}; pass --layout with this \
                     keyboard's VIA definition file (built in: V5 Max ISO, 3434:0951)",
                    device.vendor_id, device.product_id
                ))
            })?;
        let layer_count = via
            .layer_count()
            .map_err(|error| SessionError::via(&error, device.is_receiver))?;
        if let Some(layer) = selected_layer
            && layer >= layer_count
        {
            return Err(SessionError::Usage(format!(
                "layer {layer} is unavailable; select 0..={}",
                layer_count - 1
            )));
        }
        let columns = usize::from(layout.columns());
        let layer_size = usize::from(layout.rows()) * columns;
        let keycodes = via
            .keymap(usize::from(layer_count) * layer_size)
            .map_err(|error| SessionError::via(&error, device.is_receiver))?;
        let mut layers = Vec::new();
        for index in 0..layer_count {
            if selected_layer.is_some_and(|selected| selected != index) {
                continue;
            }
            let mut keys = Vec::with_capacity(layer_size);
            for row in 0..layout.rows() {
                for column in 0..layout.columns() {
                    let offset = usize::from(index) * layer_size
                        + usize::from(row) * columns
                        + usize::from(column);
                    keys.push(KeyOutput {
                        row,
                        column,
                        legend: layout.legend(row, column).map(str::to_owned),
                        is_physical: layout.has_key(row, column),
                        keycode: u16::from(keycodes[offset]),
                    });
                }
            }
            layers.push(LayerOutput { index, keys });
        }
        Ok(KeymapOutput {
            device: device_output(&device),
            layer_count,
            rows: layout.rows(),
            columns: layout.columns(),
            layers,
        })
    })();
    via.into_transport().complete(result)
}

fn print_grid(output: &KeymapOutput) {
    let layers: Vec<_> = output
        .layers
        .iter()
        .map(|layer| {
            (
                layer.index,
                layer.keys.iter().map(cell_text).collect::<Vec<_>>(),
            )
        })
        .collect();
    let width = layers
        .iter()
        .flat_map(|(_, cells)| cells)
        .map(|cell| cell.chars().count())
        .max()
        .unwrap_or(0)
        + 1;
    let has_legends = output
        .layers
        .iter()
        .flat_map(|layer| &layer.keys)
        .any(|key| key.legend.is_some());
    if has_legends {
        println!(
            "Physical FR legends = QMK name and raw hexadecimal keycode; grid uses matrix rows/columns."
        );
    } else {
        println!(
            "(row,column) = QMK name and raw hexadecimal keycode; the layout file has no legends."
        );
    }
    for (index, cells) in layers {
        println!("Layer {index}");
        for (row, cells) in cells.chunks(usize::from(output.columns)).enumerate() {
            print!("r{row} ");
            for cell in cells {
                print!("{cell:<width$}");
            }
            println!();
        }
    }
}

fn cell_text(key: &KeyOutput) -> String {
    let assignment = assignment_text(key.keycode);
    match &key.legend {
        Some(legend) => format!("{legend}={assignment}"),
        None if key.is_physical || key.keycode != 0 => {
            format!("({},{})={assignment}", key.row, key.column)
        }
        None => String::new(),
    }
}

/// QMK's name before the raw value; the raw value alone when QMK gives the keycode no name.
fn assignment_text(raw_keycode: u16) -> String {
    match Keycode::from(raw_keycode).name() {
        Some(name) => format!("{name} 0x{raw_keycode:04x}"),
        None => format!("0x{raw_keycode:04x}"),
    }
}
