//! What `read_keymap` returns: every layer's raw keycodes and their QMK names on the matrix
//! grid, with each physical key's place on the keyboard and its built-in legends.
//! Only reads.

use openkeychron_hid::{HidTransport, KeychronDevice};
use openkeychron_via::{DeviceRegistry, Keycode, Legends, Rectangle, RegisteredLayout, Via};
use serde::Serialize;

use crate::error::CommandError;

#[derive(Debug, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Keymap {
    pub layer_count: u8,
    pub rows: u8,
    pub columns: u8,
    pub layers: Vec<Layer>,
}

#[derive(Debug, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Layer {
    pub index: u8,
    /// Row by row, `columns` keys per row.
    pub keys: Vec<Key>,
}

#[derive(Debug, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Key {
    pub row: u8,
    pub column: u8,
    /// The physical French legend, which names the position, not what the key now types.
    pub legend: Option<String>,
    /// The keycap's Shift legend, such as `2` on the `é` key.
    pub shift_legend: Option<String>,
    /// The keycap's AltGr legend, such as `~` on the `é` key.
    pub alt_gr_legend: Option<String>,
    /// False for matrix slots no physical key uses.
    pub is_physical: bool,
    /// Where the key sits, in key units from the keyboard's top-left corner. Absent for a
    /// matrix slot no physical key uses.
    pub bounds: Option<Bounds>,
    /// A second rectangle that, with `bounds`, makes a shape such as ISO Enter.
    pub secondary_bounds: Option<Bounds>,
    pub keycode: u16,
    /// QMK's name for `keycode`, such as `LCTL(C)`. Absent for a custom or unknown keycode:
    /// show `keycode` then.
    pub name: Option<String>,
}

/// A rectangle in key units: 1.0 is one standard key.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Bounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl From<Rectangle> for Bounds {
    fn from(rectangle: Rectangle) -> Self {
        Self {
            x: rectangle.x,
            y: rectangle.y,
            width: rectangle.width,
            height: rectangle.height,
        }
    }
}

/// What the layout says about one physical key; definitions from a file carry no legends.
struct Placement {
    bounds: Rectangle,
    secondary_bounds: Option<Rectangle>,
    legends: Option<Legends>,
}

fn placement(layout: &RegisteredLayout, row: u8, column: u8) -> Option<Placement> {
    match layout {
        RegisteredLayout::BuiltIn(layout) => layout.key(row, column).map(|key| Placement {
            bounds: key.bounds,
            secondary_bounds: key.secondary_bounds,
            legends: Some(key.legends),
        }),
        RegisteredLayout::Definition(definition) => {
            definition.key(row, column).map(|key| Placement {
                bounds: key.bounds,
                secondary_bounds: key.secondary_bounds,
                legends: None,
            })
        }
    }
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
        .map(|((row, column), keycode)| {
            let placement = placement(layout, row, column);
            let legends = placement.as_ref().and_then(|placement| placement.legends);
            Key {
                row,
                column,
                legend: legends.map(|legends| legends.base.to_owned()),
                shift_legend: legends.and_then(|legends| legends.shift).map(str::to_owned),
                alt_gr_legend: legends
                    .and_then(|legends| legends.alt_gr)
                    .map(str::to_owned),
                is_physical: placement.is_some(),
                bounds: placement.as_ref().map(|placement| placement.bounds.into()),
                secondary_bounds: placement
                    .and_then(|placement| placement.secondary_bounds)
                    .map(Bounds::from),
                keycode: u16::from(*keycode),
                name: keycode.name(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use openkeychron_via::{RegisteredLayout, ViaDefinition};

    use super::{Bounds, Key, Keymap, keys, read};
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
                shift_legend: None,
                alt_gr_legend: None,
                is_physical: true,
                bounds: Some(Bounds {
                    x: 1.5,
                    y: 2.25,
                    width: 1.0,
                    height: 1.0,
                }),
                secondary_bounds: None,
                keycode: 0x0014,
                name: Some("Q".to_owned()),
            }
        );
        assert_eq!(keymap.layers[0].keys[1].legend, None);
        assert_eq!(keymap.layers[0].keys[1].keycode, 0xdead);
        assert_eq!(keymap.layers[1].index, 1);
        assert_eq!(keymap.layers[1].keys[113].keycode, 0xffff);
        assert_eq!(replay.finish(), vec![]);
    }

    fn read_synthetic_keymap() -> Keymap {
        let mut replay = replay("cli-keymap.toml");
        read(lend(&mut replay.transport), &replay.device).unwrap()
    }

    #[test]
    fn gives_a_matrix_slot_without_a_physical_key_no_bounds() {
        let keymap = read_synthetic_keymap();
        let unused = &keymap.layers[0].keys[1];
        assert_eq!(
            (unused.is_physical, unused.bounds, unused.secondary_bounds),
            (false, None, None)
        );
    }

    #[test]
    fn names_each_keycode_as_qmk_does_next_to_its_raw_value() {
        let keymap = read_synthetic_keymap();
        let named = [(0, 0), (0, 39), (0, 58), (1, 0)].map(|(layer, slot)| {
            let key: &Key = &keymap.layers[layer].keys[slot];
            (key.keycode, key.name.clone())
        });
        assert_eq!(
            named,
            [
                (0x0029, Some("ESC".to_owned())),
                (0x0014, Some("Q".to_owned())),
                (0x0004, Some("A".to_owned())),
                (0x0001, Some("TRNS".to_owned())),
            ]
        );
    }

    #[test]
    fn gives_a_custom_or_unknown_keycode_no_name() {
        let keymap = read_synthetic_keymap();
        let unnamed = [(0, 1), (0, 113), (1, 67), (1, 113)].map(|(layer, slot)| {
            let key: &Key = &keymap.layers[layer].keys[slot];
            (key.keycode, key.name.clone())
        });
        assert_eq!(
            unnamed,
            [
                (0xdead, None),
                (0xbeef, None),
                (0x7e00, None),
                (0xffff, None)
            ]
        );
    }

    #[test]
    fn gives_iso_enter_its_second_rectangle() {
        let keymap = read_synthetic_keymap();
        let enter = &keymap.layers[0].keys[2 * 19 + 13];
        assert_eq!(enter.legend.as_deref(), Some("Enter"));
        assert_eq!(
            (enter.bounds, enter.secondary_bounds),
            (
                Some(Bounds {
                    x: 13.75,
                    y: 2.25,
                    width: 1.25,
                    height: 2.0,
                }),
                Some(Bounds {
                    x: 13.5,
                    y: 2.25,
                    width: 1.5,
                    height: 1.0,
                })
            )
        );
    }

    #[test]
    fn gives_the_french_shift_and_alt_gr_legends() {
        let keymap = read_synthetic_keymap();
        let e_acute = &keymap.layers[0].keys[19 + 2];
        assert_eq!(
            (
                e_acute.legend.as_deref(),
                e_acute.shift_legend.as_deref(),
                e_acute.alt_gr_legend.as_deref()
            ),
            (Some("é"), Some("2"), Some("~"))
        );
    }

    #[test]
    fn places_the_keys_of_a_via_definition_without_legends() {
        let text = std::fs::read_to_string(crate::test_support::synthetic_path(
            "via-definition-second-model.json",
        ))
        .unwrap();
        let definition = ViaDefinition::from_json(&text).unwrap();
        let first = definition.keys[0];
        let layout = RegisteredLayout::Definition(definition);
        let layer_size = usize::from(layout.rows()) * usize::from(layout.columns());
        let placed = keys(&layout, &vec![0u16.into(); layer_size]);
        let key = placed
            .iter()
            .find(|key| [key.row, key.column] == first.matrix)
            .unwrap();
        assert_eq!(
            (
                key.bounds,
                key.legend.as_deref(),
                key.shift_legend.as_deref()
            ),
            (
                Some(Bounds {
                    x: first.bounds.x,
                    y: first.bounds.y,
                    width: first.bounds.width,
                    height: first.bounds.height,
                }),
                None,
                None
            )
        );
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
