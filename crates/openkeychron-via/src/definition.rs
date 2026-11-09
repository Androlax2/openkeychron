//! Physical layouts read from VIA definition files, such as the ones Keychron publishes for
//! each model in `keyboards/keychron/*/via_json/`.
//!
//! `layouts.keymap` is in keyboard-layout-editor (KLE) form. It is read with the rules of
//! VIA's own reader, [`kle-parser.ts`](https://github.com/the-via/reader/blob/c26009992a7a2e9776720ebf5644a50f8e92c7a5/src/kle-parser.ts):
//! the first label is `row,column`, the tenth is an encoder `e<index>`, and the fourth is a
//! layout option. Features this crate cannot represent yet (decals, label alignment, layout
//! options) are rejected, never approximated.
//! See `docs/protocol/via-definitions.md`.

use std::collections::BTreeSet;

use serde::Deserialize;
use serde_json::{Map, Value};
use thiserror::Error;

use crate::Rectangle;

/// One keyboard model's physical layout, as its VIA definition file describes it.
/// VIA definitions name matrix positions, not legends, so keys carry no display text.
#[derive(Debug, Clone, PartialEq)]
pub struct ViaDefinition {
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub rows: u8,
    pub columns: u8,
    /// Physical keys in the file's order, not matrix order.
    pub keys: Vec<DefinitionKey>,
}

/// A physical key from a VIA definition, including an encoder's press.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DefinitionKey {
    /// VIA matrix row and column, not a USB keycode.
    pub matrix: [u8; 2],
    /// Absolute layout coordinates, with the layout's top-left corner at (0, 0).
    pub bounds: Rectangle,
    /// An additional rectangle in absolute layout coordinates, for shapes like ISO Enter.
    pub secondary_bounds: Option<Rectangle>,
    /// Encoder index associated with this key's press.
    pub encoder: Option<u8>,
    /// How the key (and its secondary rectangle) is turned after being placed by `bounds`.
    /// `None` for a key that is not rotated.
    pub rotation: Option<KeyRotation>,
}

/// A rotation of a key's rectangles about a point, in the same absolute layout coordinates
/// as [`DefinitionKey::bounds`]. `bounds` is the key before it is turned.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyRotation {
    /// Clockwise on screen (y grows downward), in degrees.
    pub angle_degrees: f32,
    pub origin_x: f32,
    pub origin_y: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DefinitionError {
    #[error("the file is not a VIA definition: {message}")]
    Json { message: String },

    #[error("{field} must be a hexadecimal string such as \"0x3434\", got {value:?}")]
    InvalidId { field: &'static str, value: String },

    #[error("matrix must have at least one row and one column")]
    EmptyMatrix,

    #[error("layout options (layouts.labels) are not supported yet")]
    LayoutOptions,

    #[error("layouts.keymap[{keymap_row}][{item}] must be a key label string or a property object")]
    UnexpectedItem { keymap_row: usize, item: usize },

    #[error(
        "layouts.keymap[{keymap_row}][{item}] uses {property:?}, which openkeychron cannot \
         represent yet (label alignment and decals are not supported)"
    )]
    UnsupportedProperty {
        keymap_row: usize,
        item: usize,
        property: String,
    },

    #[error("layouts.keymap[{keymap_row}][{item}] property {property:?} must be a number")]
    InvalidProperty {
        keymap_row: usize,
        item: usize,
        property: String,
    },

    #[error(
        "layouts.keymap[{keymap_row}][{item}] label {label:?} must start with a matrix \
         position \"row,column\""
    )]
    InvalidMatrixLabel {
        keymap_row: usize,
        item: usize,
        label: String,
    },

    #[error(
        "key ({}, {}) is outside the {rows} × {columns} matrix",
        matrix[0], matrix[1]
    )]
    MatrixOutOfRange {
        matrix: [u8; 2],
        rows: u8,
        columns: u8,
    },

    #[error("two keys use matrix position ({}, {})", matrix[0], matrix[1])]
    DuplicateKey { matrix: [u8; 2] },

    #[error(
        "layouts.keymap[{keymap_row}][{item}] encoder label {label:?} must be \"e\" and an index"
    )]
    InvalidEncoder {
        keymap_row: usize,
        item: usize,
        label: String,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DefinitionFile {
    name: String,
    vendor_id: String,
    product_id: String,
    matrix: MatrixSize,
    layouts: Layouts,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MatrixSize {
    rows: u8,
    cols: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Layouts {
    keymap: Vec<Vec<Value>>,
    labels: Option<Value>,
}

/// Label positions in a KLE key string split on `\n`, as VIA's reader numbers them before
/// label alignment (which is rejected, so the default alignment always applies).
const MATRIX_LABEL: usize = 0;
const LAYOUT_OPTION_LABEL: usize = 3;
const ENCODER_LABEL: usize = 9;

/// Properties VIA's reader honours that this crate cannot represent. VIA ignores every
/// other property except the geometry ones, so they are ignored here too.
const UNSUPPORTED_PROPERTIES: &[&str] = &["a", "d"];

impl ViaDefinition {
    /// Reads a VIA definition. Only the identity, matrix size and `layouts` are used; other
    /// top-level fields, such as `menus` and `customKeycodes`, are ignored.
    pub fn from_json(text: &str) -> Result<Self, DefinitionError> {
        let file: DefinitionFile =
            serde_json::from_str(text).map_err(|error| DefinitionError::Json {
                message: error.to_string(),
            })?;
        if file
            .layouts
            .labels
            .is_some_and(|labels| labels != Value::Array(Vec::new()))
        {
            return Err(DefinitionError::LayoutOptions);
        }
        let (rows, columns) = (file.matrix.rows, file.matrix.cols);
        if rows == 0 || columns == 0 {
            return Err(DefinitionError::EmptyMatrix);
        }
        let mut keys = read_keymap(&file.layouts.keymap)?;
        check_matrix(&keys, rows, columns)?;
        move_origin_to_top_left(&mut keys);
        Ok(Self {
            name: file.name,
            vendor_id: parse_id("vendorId", &file.vendor_id)?,
            product_id: parse_id("productId", &file.product_id)?,
            rows,
            columns,
            keys,
        })
    }

    /// Looks up a physical key; unused or out-of-range matrix positions return `None`.
    #[must_use]
    pub fn key(&self, row: u8, column: u8) -> Option<&DefinitionKey> {
        self.keys.iter().find(|key| key.matrix == [row, column])
    }
}

fn parse_id(field: &'static str, value: &str) -> Result<u16, DefinitionError> {
    value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .filter(|digits| digits.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .and_then(|digits| u16::from_str_radix(digits, 16).ok())
        .ok_or_else(|| DefinitionError::InvalidId {
            field,
            value: value.to_owned(),
        })
}

/// What the next key inherits from the property objects before it. Rotation carries from
/// row to row, as in VIA's reader, until another object changes it.
struct Cursor {
    x: f32,
    y: f32,
    angle_degrees: f32,
    origin_x: f32,
    origin_y: f32,
    width: f32,
    height: f32,
    x2: Option<f32>,
    y2: Option<f32>,
    width2: Option<f32>,
    height2: Option<f32>,
}

impl Cursor {
    fn reset_key_size(&mut self) {
        self.width = 1.0;
        self.height = 1.0;
        self.x2 = None;
        self.y2 = None;
        self.width2 = None;
        self.height2 = None;
    }
}

fn read_keymap(keymap: &[Vec<Value>]) -> Result<Vec<DefinitionKey>, DefinitionError> {
    let mut cursor = Cursor {
        x: 0.0,
        y: 0.0,
        angle_degrees: 0.0,
        origin_x: 0.0,
        origin_y: 0.0,
        width: 1.0,
        height: 1.0,
        x2: None,
        y2: None,
        width2: None,
        height2: None,
    };
    let mut keys = Vec::new();
    for (keymap_row, items) in keymap.iter().enumerate() {
        for (item, value) in items.iter().enumerate() {
            match value {
                Value::Object(properties) => {
                    apply_properties(&mut cursor, properties, keymap_row, item)?;
                }
                Value::String(label) => {
                    keys.push(place_key(&cursor, label, keymap_row, item)?);
                    cursor.x += cursor.width;
                    cursor.reset_key_size();
                }
                _ => return Err(DefinitionError::UnexpectedItem { keymap_row, item }),
            }
        }
        cursor.x = 0.0;
        cursor.y += 1.0;
        cursor.reset_key_size();
    }
    Ok(keys)
}

fn apply_properties(
    cursor: &mut Cursor,
    properties: &Map<String, Value>,
    keymap_row: usize,
    item: usize,
) -> Result<(), DefinitionError> {
    let number = |property: &str| -> Result<Option<f32>, DefinitionError> {
        properties
            .get(property)
            .map(|value| {
                f32::deserialize(value).map_err(|_| DefinitionError::InvalidProperty {
                    keymap_row,
                    item,
                    property: property.to_owned(),
                })
            })
            .transpose()
    };
    if let Some(property) = properties
        .keys()
        .find(|property| UNSUPPORTED_PROPERTIES.contains(&property.as_str()))
    {
        return Err(DefinitionError::UnsupportedProperty {
            keymap_row,
            item,
            property: property.clone(),
        });
    }
    if let Some(angle) = number("r")? {
        cursor.angle_degrees = angle;
    }
    let origin_x = number("rx")?;
    let origin_y = number("ry")?;
    if let Some(origin_x) = origin_x {
        cursor.origin_x = origin_x;
    }
    if let Some(origin_y) = origin_y {
        cursor.origin_y = origin_y;
    }
    // VIA moves to the rotation origin's row whenever either coordinate is given. The column
    // is not moved: every row starts at x = 0, and a key's x is added to the origin's.
    if origin_x.is_some() || origin_y.is_some() {
        cursor.y = cursor.origin_y;
    }
    if let Some(y) = number("y")? {
        cursor.y += y;
    }
    if let Some(x) = number("x")? {
        cursor.x += x;
    }
    if let Some(width) = number("w")? {
        cursor.width = width;
    }
    if let Some(height) = number("h")? {
        cursor.height = height;
    }
    for (property, slot) in [
        ("x2", &mut cursor.x2),
        ("y2", &mut cursor.y2),
        ("w2", &mut cursor.width2),
        ("h2", &mut cursor.height2),
    ] {
        if let Some(value) = number(property)? {
            *slot = Some(value);
        }
    }
    Ok(())
}

fn place_key(
    cursor: &Cursor,
    label: &str,
    keymap_row: usize,
    item: usize,
) -> Result<DefinitionKey, DefinitionError> {
    let labels: Vec<&str> = label.split('\n').map(str::trim).collect();
    let label_at = |index: usize| labels.get(index).copied().unwrap_or("");
    if !label_at(LAYOUT_OPTION_LABEL).is_empty() {
        return Err(DefinitionError::LayoutOptions);
    }
    let matrix = parse_matrix(label_at(MATRIX_LABEL)).ok_or_else(|| {
        DefinitionError::InvalidMatrixLabel {
            keymap_row,
            item,
            label: label_at(MATRIX_LABEL).to_owned(),
        }
    })?;
    let encoder =
        match label_at(ENCODER_LABEL) {
            "" => None,
            encoder_label => Some(parse_encoder(encoder_label).ok_or_else(|| {
                DefinitionError::InvalidEncoder {
                    keymap_row,
                    item,
                    label: encoder_label.to_owned(),
                }
            })?),
        };
    let has_secondary = cursor.x2.is_some()
        || cursor.y2.is_some()
        || cursor.width2.is_some()
        || cursor.height2.is_some();
    let left = cursor.x + cursor.origin_x;
    Ok(DefinitionKey {
        matrix,
        bounds: Rectangle {
            x: left,
            y: cursor.y,
            width: cursor.width,
            height: cursor.height,
        },
        secondary_bounds: has_secondary.then(|| Rectangle {
            x: left + cursor.x2.unwrap_or(0.0),
            y: cursor.y + cursor.y2.unwrap_or(0.0),
            width: cursor.width2.unwrap_or(cursor.width),
            height: cursor.height2.unwrap_or(cursor.height),
        }),
        encoder,
        rotation: (cursor.angle_degrees != 0.0).then_some(KeyRotation {
            angle_degrees: cursor.angle_degrees,
            origin_x: cursor.origin_x,
            origin_y: cursor.origin_y,
        }),
    })
}

fn parse_matrix(label: &str) -> Option<[u8; 2]> {
    let (row, column) = label.split_once(',')?;
    Some([row.trim().parse().ok()?, column.trim().parse().ok()?])
}

fn parse_encoder(label: &str) -> Option<u8> {
    let index = label.strip_prefix(['e', 'E'])?;
    if !index.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    index.parse().ok()
}

fn check_matrix(keys: &[DefinitionKey], rows: u8, columns: u8) -> Result<(), DefinitionError> {
    let mut seen = BTreeSet::new();
    for key in keys {
        let [row, column] = key.matrix;
        if row >= rows || column >= columns {
            return Err(DefinitionError::MatrixOutOfRange {
                matrix: key.matrix,
                rows,
                columns,
            });
        }
        if !seen.insert(key.matrix) {
            return Err(DefinitionError::DuplicateKey { matrix: key.matrix });
        }
    }
    Ok(())
}

/// VIA draws a layout from the top-left corner of its keys as they appear after rotation,
/// not from KLE's origin, so a definition whose first key is offset or turned still starts at
/// (0, 0).
fn move_origin_to_top_left(keys: &mut [DefinitionKey]) {
    let corners = || keys.iter().flat_map(rotated_corners);
    let left = corners()
        .map(|corner| corner.0)
        .fold(f64::INFINITY, f64::min);
    let top = corners()
        .map(|corner| corner.1)
        .fold(f64::INFINITY, f64::min);
    if !left.is_finite() || !top.is_finite() {
        return;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "layout units are small f32 values"
    )]
    let (left, top) = (left as f32, top as f32);
    for key in keys.iter_mut() {
        for rectangle in std::iter::once(&mut key.bounds).chain(key.secondary_bounds.as_mut()) {
            rectangle.x -= left;
            rectangle.y -= top;
        }
        if let Some(rotation) = key.rotation.as_mut() {
            rotation.origin_x -= left;
            rotation.origin_y -= top;
        }
    }
}

/// The corners of a key's rectangles after its rotation. Computed in `f64` so a quarter turn
/// does not leave rounding noise in the layout's offset.
fn rotated_corners(key: &DefinitionKey) -> Vec<(f64, f64)> {
    let (sin, cos) = key.rotation.map_or((0.0, 1.0), |rotation| {
        f64::from(rotation.angle_degrees).to_radians().sin_cos()
    });
    let origin = key.rotation.map_or((0.0, 0.0), |rotation| {
        (f64::from(rotation.origin_x), f64::from(rotation.origin_y))
    });
    // VIA turns the box around both rectangles, not each rectangle, so a turned ISO Enter
    // measures the same here as in VIA.
    let rectangles: Vec<Rectangle> = std::iter::once(key.bounds)
        .chain(key.secondary_bounds)
        .collect();
    let left = rectangles
        .iter()
        .map(|r| f64::from(r.x))
        .fold(f64::INFINITY, f64::min);
    let top = rectangles
        .iter()
        .map(|r| f64::from(r.y))
        .fold(f64::INFINITY, f64::min);
    let right = rectangles
        .iter()
        .map(|r| f64::from(r.x + r.width))
        .fold(f64::NEG_INFINITY, f64::max);
    let bottom = rectangles
        .iter()
        .map(|r| f64::from(r.y + r.height))
        .fold(f64::NEG_INFINITY, f64::max);
    [(left, top), (right, top), (left, bottom), (right, bottom)]
        .into_iter()
        .map(|(x, y)| {
            if key.rotation.is_none() {
                return (x, y);
            }
            let (offset_x, offset_y) = (x - origin.0, y - origin.1);
            (
                origin.0 + offset_x * cos - offset_y * sin,
                origin.1 + offset_x * sin + offset_y * cos,
            )
        })
        .collect()
}
