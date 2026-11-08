//! `keymap`: every layer's raw keycodes on the matrix grid, with the built-in physical legends.
//! Only reads.

use openkeychron_hid::{HidTransport, KeychronDevice};
use openkeychron_via::{DeviceRegistry, Keycode, RegisteredLayout, Via};
use serde::Serialize;

use crate::error::CommandError;

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Keymap {
    pub layer_count: u8,
    pub rows: u8,
    pub columns: u8,
    pub layers: Vec<Layer>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Layer {
    pub index: u8,
    /// Row by row, `columns` keys per row.
    pub keys: Vec<Key>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Key {
    pub row: u8,
    pub column: u8,
    /// The physical French legend, which names the position, not what the key now types.
    pub legend: Option<String>,
    /// False for matrix slots no physical key uses.
    pub is_physical: bool,
    pub keycode: u16,
}

pub fn read<T: HidTransport>(
    transport: T,
    device: &KeychronDevice,
) -> Result<Keymap, CommandError> {
    if device.is_receiver {
        return Err(CommandError::NeedsCable {
            reading: "the keymap",
        });
    }
    let registry = DeviceRegistry::built_in();
    let layout =
        registry
            .layout(device.vendor_id, device.product_id)
            .ok_or(CommandError::NoLayout {
                vendor_id: device.vendor_id,
                product_id: device.product_id,
            })?;
    let mut via = Via::new(transport);
    let via_error = |error| CommandError::via(&error, device.is_receiver);
    let layer_count = via.layer_count().map_err(via_error)?;
    let layer_size = usize::from(layout.rows()) * usize::from(layout.columns());
    let keycodes = via
        .keymap(usize::from(layer_count) * layer_size)
        .map_err(via_error)?;
    let layers = (0..layer_count)
        .zip(keycodes.chunks(layer_size))
        .map(|(index, layer_keycodes)| Layer {
            index,
            keys: keys(layout, layer_keycodes),
        })
        .collect();
    Ok(Keymap {
        layer_count,
        rows: layout.rows(),
        columns: layout.columns(),
        layers,
    })
}

fn keys(layout: &RegisteredLayout, keycodes: &[Keycode]) -> Vec<Key> {
    let positions =
        (0..layout.rows()).flat_map(|row| (0..layout.columns()).map(move |column| (row, column)));
    positions
        .zip(keycodes)
        .map(|((row, column), keycode)| Key {
            row,
            column,
            legend: layout.legend(row, column).map(str::to_owned),
            is_physical: layout.has_key(row, column),
            keycode: u16::from(*keycode),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Key, read};
    use crate::test_support::{lend, load_fixture, replay};

    #[test]
    fn reads_every_layer_with_physical_legends_and_raw_keycodes() {
        let mut replay = replay("cli-keymap.toml");
        let keymap = read(lend(&mut replay.transport), &replay.device).unwrap();
        assert_eq!(
            (keymap.layer_count, keymap.rows, keymap.columns),
            (2, 6, 19)
        );
        assert_eq!(
            keymap
                .layers
                .iter()
                .map(|layer| layer.keys.len())
                .collect::<Vec<_>>(),
            [114, 114]
        );
        assert_eq!(
            keymap.layers[0].keys[39],
            Key {
                row: 2,
                column: 1,
                legend: Some("A".to_owned()),
                is_physical: true,
                keycode: 0x0014,
            }
        );
        assert_eq!(keymap.layers[0].keys[1].legend, None);
        assert_eq!(keymap.layers[0].keys[1].keycode, 0xdead);
        assert_eq!(keymap.layers[1].index, 1);
        assert_eq!(keymap.layers[1].keys[113].keycode, 0xffff);
        assert_eq!(replay.finish(), vec![]);
    }

    #[test]
    fn refuses_a_receiver_before_sending_anything() {
        let mut replay = replay("cli-info.toml");
        let error = read(lend(&mut replay.transport), &replay.device).unwrap_err();
        assert_eq!(
            error.to_string(),
            "the keymap needs the keyboard on a USB cable: a receiver does not identify the \
             keyboard behind it"
        );
        assert_eq!(replay.finish(), load_fixture("cli-info.toml").events);
    }

    #[test]
    fn refuses_a_keyboard_without_a_built_in_layout_before_sending_anything() {
        let mut replay = replay("cli-keymap-second-model.toml");
        let error = read(lend(&mut replay.transport), &replay.device).unwrap_err();
        assert_eq!(
            error.to_string(),
            "this build has no keyboard layout for 3434:0950, so it cannot place its keys"
        );
        assert_eq!(
            replay.finish(),
            load_fixture("cli-keymap-second-model.toml").events
        );
    }
}
