//! Read-only keymap display. Physical French legends name positions, not live assignments.

use std::path::Path;

use clap::Subcommand;
use openkeychron_via::{V5_MAX_FR_ISO, Via};
use serde::Serialize;

use crate::session::{Session, SessionError};
use crate::{DeviceOutput, device_output};

#[derive(Debug, Subcommand)]
pub enum KeymapAction {
    /// Print each layer as a French-position grid with raw keycodes. Only reads.
    Show {
        /// Show only this zero-based layer (all layers are still read).
        #[arg(long)]
        layer: Option<u8>,
    },
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
    legend: Option<&'static str>,
    keycode: u16,
}

pub fn run(
    action: &KeymapAction,
    fixture: Option<&Path>,
    record: Option<&Path>,
    device: Option<&Path>,
    json: bool,
) -> Result<(), SessionError> {
    let KeymapAction::Show { layer } = action;
    let output = query(Session::start(fixture, record, device)?, *layer)?;
    if json {
        return crate::print_json(&output);
    }
    print_grid(&output);
    Ok(())
}

fn query(session: Session, selected_layer: Option<u8>) -> Result<KeymapOutput, SessionError> {
    let device = session.device().clone();
    let mut via = Via::new(session);
    let result = (|| {
        // Only this cable identity has known geometry; a receiver does not identify its keyboard.
        if device.is_receiver || device.vendor_id != 0x3434 || device.product_id != 0x0951 {
            return Err(SessionError::Usage(format!(
                "keymap show requires the V5 Max ISO keyboard (3434:0951) on a USB cable; \
                 got {:04x}:{:04x}",
                device.vendor_id, device.product_id
            )));
        }
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
        let layout = V5_MAX_FR_ISO;
        let columns = usize::from(layout.columns);
        let layer_size = usize::from(layout.rows) * columns;
        let keycodes = via
            .keymap(usize::from(layer_count) * layer_size)
            .map_err(|error| SessionError::via(&error, device.is_receiver))?;
        let mut layers = Vec::new();
        for index in 0..layer_count {
            if selected_layer.is_some_and(|selected| selected != index) {
                continue;
            }
            let mut keys = Vec::with_capacity(layer_size);
            for row in 0..layout.rows {
                for column in 0..layout.columns {
                    let offset = usize::from(index) * layer_size
                        + usize::from(row) * columns
                        + usize::from(column);
                    keys.push(KeyOutput {
                        row,
                        column,
                        legend: layout.key(row, column).map(|key| key.legends.base),
                        keycode: u16::from(keycodes[offset]),
                    });
                }
            }
            layers.push(LayerOutput { index, keys });
        }
        Ok(KeymapOutput {
            device: device_output(&device),
            layer_count,
            rows: layout.rows,
            columns: layout.columns,
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
    println!("Physical FR legends = raw hexadecimal keycodes; grid uses matrix rows/columns.");
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
    match key.legend {
        Some(legend) => format!("{legend}=0x{:04x}", key.keycode),
        None if key.keycode != 0 => {
            format!("({},{})=0x{:04x}", key.row, key.column, key.keycode)
        }
        None => String::new(),
    }
}
