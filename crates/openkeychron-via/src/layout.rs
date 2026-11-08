//! Physical keyboard geometry and locale-specific display legends, independent of keymaps.
//!
//! Sources at Keychron QMK revision `2b493ff95f2665db719b1e95c91babb5036e72f3`:
//! - [ISO VIA geometry](https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/keyboards/keychron/v5_max/via_json/v5_max_iso_encoder.json).
//! - [French aliases](https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/keymap_extras/keymap_french.h), copyright 2025 QMK, GPL-2.0-or-later.
//! - [Default ISO key roles](https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/keyboards/keychron/v5_max/iso_encoder/keymaps/keychron/keymap.c), copyright 2024–2025 Keychron, GPL-2.0-or-later.
//!
//! Geometry and legend facts are restated here, not firmware implementation code.
//! See `docs/protocol/v5-max-fr-iso-layout.md` for conversion and display limitations.

/// A rectangle in keyboard units, positioned relative to the layout's top-left corner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rectangle {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Physical French legends, not text output or the current dynamic key assignment.
/// Letter legends use uppercase without a separate Shift legend. Accent symbols may
/// represent dead keys; these strings must not be used as a text-input implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Legends {
    pub base: &'static str,
    pub shift: Option<&'static str>,
    pub alt_gr: Option<&'static str>,
}

/// A physical key, including the encoder's press when present.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicalKey {
    /// VIA matrix row and column, not a USB keycode.
    pub matrix: [u8; 2],
    pub bounds: Rectangle,
    /// An additional rectangle in absolute layout coordinates, for shapes like ISO Enter.
    pub secondary_bounds: Option<Rectangle>,
    /// Encoder index associated with this key's press; rotation maps are separate.
    pub encoder: Option<u8>,
    pub legends: Legends,
}

/// Physical keys in a matrix; unused matrix positions have no corresponding key.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyboardLayout {
    pub rows: u8,
    pub columns: u8,
    pub keys: &'static [PhysicalKey],
}

impl KeyboardLayout {
    /// Looks up a physical key; unused or out-of-range matrix positions return `None`.
    #[must_use]
    pub fn key(&self, row: u8, column: u8) -> Option<&PhysicalKey> {
        if row >= self.rows || column >= self.columns {
            return None;
        }
        self.keys.iter().find(|key| key.matrix == [row, column])
    }
}

/// Keychron V5 Max ISO (`3434:0951`) with French AZERTY display legends.
pub const V5_MAX_FR_ISO: KeyboardLayout = KeyboardLayout {
    rows: 6,
    columns: 19,
    keys: &[
        key([0, 0], [0.0, 0.0, 1.0, 1.0], ("Esc", None, None)),
        key([0, 2], [2.0, 0.0, 1.0, 1.0], ("F1", None, None)),
        key([0, 3], [3.0, 0.0, 1.0, 1.0], ("F2", None, None)),
        key([0, 4], [4.0, 0.0, 1.0, 1.0], ("F3", None, None)),
        key([0, 5], [5.0, 0.0, 1.0, 1.0], ("F4", None, None)),
        key([0, 7], [6.5, 0.0, 1.0, 1.0], ("F5", None, None)),
        key([0, 8], [7.5, 0.0, 1.0, 1.0], ("F6", None, None)),
        key([0, 9], [8.5, 0.0, 1.0, 1.0], ("F7", None, None)),
        key([0, 10], [9.5, 0.0, 1.0, 1.0], ("F8", None, None)),
        key([0, 11], [11.0, 0.0, 1.0, 1.0], ("F9", None, None)),
        key([0, 12], [12.0, 0.0, 1.0, 1.0], ("F10", None, None)),
        key([0, 13], [13.0, 0.0, 1.0, 1.0], ("F11", None, None)),
        key([0, 14], [14.0, 0.0, 1.0, 1.0], ("F12", None, None)),
        key([0, 15], [15.25, 0.0, 1.0, 1.0], ("Del", None, None)),
        key([0, 16], [16.25, 0.0, 1.0, 1.0], ("Home", None, None)),
        key([0, 17], [17.25, 0.0, 1.0, 1.0], ("End", None, None)),
        PhysicalKey {
            encoder: Some(0),
            ..key([0, 18], [18.5, 0.0, 1.0, 1.0], ("Knob", None, None))
        },
        key([1, 0], [0.0, 1.25, 1.0, 1.0], ("²", None, None)),
        key([1, 1], [1.0, 1.25, 1.0, 1.0], ("&", Some("1"), None)),
        key([1, 2], [2.0, 1.25, 1.0, 1.0], ("é", Some("2"), Some("~"))),
        key([1, 3], [3.0, 1.25, 1.0, 1.0], ("\"", Some("3"), Some("#"))),
        key([1, 4], [4.0, 1.25, 1.0, 1.0], ("'", Some("4"), Some("{"))),
        key([1, 5], [5.0, 1.25, 1.0, 1.0], ("(", Some("5"), Some("["))),
        key([1, 6], [6.0, 1.25, 1.0, 1.0], ("-", Some("6"), Some("|"))),
        key([1, 7], [7.0, 1.25, 1.0, 1.0], ("è", Some("7"), Some("`"))),
        key([1, 8], [8.0, 1.25, 1.0, 1.0], ("_", Some("8"), Some("\\"))),
        key([1, 9], [9.0, 1.25, 1.0, 1.0], ("ç", Some("9"), None)),
        key([1, 10], [10.0, 1.25, 1.0, 1.0], ("à", Some("0"), Some("@"))),
        key([1, 11], [11.0, 1.25, 1.0, 1.0], (")", Some("°"), Some("]"))),
        key([1, 12], [12.0, 1.25, 1.0, 1.0], ("=", Some("+"), Some("}"))),
        key([1, 13], [13.0, 1.25, 2.0, 1.0], ("Backspace", None, None)),
        key([1, 15], [15.5, 1.25, 1.0, 1.0], ("Num Lock", None, None)),
        key([1, 16], [16.5, 1.25, 1.0, 1.0], ("Num /", None, None)),
        key([1, 17], [17.5, 1.25, 1.0, 1.0], ("Num *", None, None)),
        key([1, 18], [18.5, 1.25, 1.0, 1.0], ("Num -", None, None)),
        key([2, 0], [0.0, 2.25, 1.5, 1.0], ("Tab", None, None)),
        key([2, 1], [1.5, 2.25, 1.0, 1.0], ("A", None, None)),
        key([2, 2], [2.5, 2.25, 1.0, 1.0], ("Z", None, None)),
        key([2, 3], [3.5, 2.25, 1.0, 1.0], ("E", None, Some("€"))),
        key([2, 4], [4.5, 2.25, 1.0, 1.0], ("R", None, None)),
        key([2, 5], [5.5, 2.25, 1.0, 1.0], ("T", None, None)),
        key([2, 6], [6.5, 2.25, 1.0, 1.0], ("Y", None, None)),
        key([2, 7], [7.5, 2.25, 1.0, 1.0], ("U", None, None)),
        key([2, 8], [8.5, 2.25, 1.0, 1.0], ("I", None, None)),
        key([2, 9], [9.5, 2.25, 1.0, 1.0], ("O", None, None)),
        key([2, 10], [10.5, 2.25, 1.0, 1.0], ("P", None, None)),
        key([2, 11], [11.5, 2.25, 1.0, 1.0], ("^", Some("¨"), None)),
        key([2, 12], [12.5, 2.25, 1.0, 1.0], ("$", Some("£"), Some("¤"))),
        PhysicalKey {
            secondary_bounds: Some(Rectangle {
                x: 13.5,
                y: 2.25,
                width: 1.5,
                height: 1.0,
            }),
            ..key([2, 13], [13.75, 2.25, 1.25, 2.0], ("Enter", None, None))
        },
        key([2, 15], [15.5, 2.25, 1.0, 1.0], ("Num 7", None, None)),
        key([2, 16], [16.5, 2.25, 1.0, 1.0], ("Num 8", None, None)),
        key([2, 17], [17.5, 2.25, 1.0, 1.0], ("Num 9", None, None)),
        key([3, 18], [18.5, 2.25, 1.0, 2.0], ("Num +", None, None)),
        key([3, 0], [0.0, 3.25, 1.75, 1.0], ("Caps Lock", None, None)),
        key([3, 1], [1.75, 3.25, 1.0, 1.0], ("Q", None, None)),
        key([3, 2], [2.75, 3.25, 1.0, 1.0], ("S", None, None)),
        key([3, 3], [3.75, 3.25, 1.0, 1.0], ("D", None, None)),
        key([3, 4], [4.75, 3.25, 1.0, 1.0], ("F", None, None)),
        key([3, 5], [5.75, 3.25, 1.0, 1.0], ("G", None, None)),
        key([3, 6], [6.75, 3.25, 1.0, 1.0], ("H", None, None)),
        key([3, 7], [7.75, 3.25, 1.0, 1.0], ("J", None, None)),
        key([3, 8], [8.75, 3.25, 1.0, 1.0], ("K", None, None)),
        key([3, 9], [9.75, 3.25, 1.0, 1.0], ("L", None, None)),
        key([3, 10], [10.75, 3.25, 1.0, 1.0], ("M", None, None)),
        key([3, 11], [11.75, 3.25, 1.0, 1.0], ("ù", Some("%"), None)),
        key([3, 12], [12.75, 3.25, 1.0, 1.0], ("*", Some("µ"), None)),
        key([3, 15], [15.5, 3.25, 1.0, 1.0], ("Num 4", None, None)),
        key([3, 16], [16.5, 3.25, 1.0, 1.0], ("Num 5", None, None)),
        key([3, 17], [17.5, 3.25, 1.0, 1.0], ("Num 6", None, None)),
        key([4, 0], [0.0, 4.25, 1.25, 1.0], ("Shift", None, None)),
        key([4, 1], [1.25, 4.25, 1.0, 1.0], ("<", Some(">"), None)),
        key([4, 2], [2.25, 4.25, 1.0, 1.0], ("W", None, None)),
        key([4, 3], [3.25, 4.25, 1.0, 1.0], ("X", None, None)),
        key([4, 4], [4.25, 4.25, 1.0, 1.0], ("C", None, None)),
        key([4, 5], [5.25, 4.25, 1.0, 1.0], ("V", None, None)),
        key([4, 6], [6.25, 4.25, 1.0, 1.0], ("B", None, None)),
        key([4, 7], [7.25, 4.25, 1.0, 1.0], ("N", None, None)),
        key([4, 8], [8.25, 4.25, 1.0, 1.0], (",", Some("?"), None)),
        key([4, 9], [9.25, 4.25, 1.0, 1.0], (";", Some("."), None)),
        key([4, 10], [10.25, 4.25, 1.0, 1.0], (":", Some("/"), None)),
        key([4, 11], [11.25, 4.25, 1.0, 1.0], ("!", Some("§"), None)),
        key([4, 12], [12.25, 4.25, 1.75, 1.0], ("Shift", None, None)),
        key([4, 14], [14.25, 4.5, 1.0, 1.0], ("↑", None, None)),
        key([4, 15], [15.5, 4.25, 1.0, 1.0], ("Num 1", None, None)),
        key([4, 16], [16.5, 4.25, 1.0, 1.0], ("Num 2", None, None)),
        key([4, 17], [17.5, 4.25, 1.0, 1.0], ("Num 3", None, None)),
        key([4, 18], [18.5, 4.25, 1.0, 2.0], ("Num Enter", None, None)),
        key([5, 0], [0.0, 5.25, 1.25, 1.0], ("Ctrl", None, None)),
        key([5, 1], [1.25, 5.25, 1.25, 1.0], ("Win", None, None)),
        key([5, 2], [2.5, 5.25, 1.25, 1.0], ("Alt", None, None)),
        key([5, 6], [3.75, 5.25, 6.25, 1.0], ("Space", None, None)),
        key([5, 10], [10.0, 5.25, 1.0, 1.0], ("AltGr", None, None)),
        key([5, 11], [11.0, 5.25, 1.0, 1.0], ("Fn", None, None)),
        key([5, 12], [12.0, 5.25, 1.0, 1.0], ("Ctrl", None, None)),
        key([5, 13], [13.25, 5.5, 1.0, 1.0], ("←", None, None)),
        key([5, 14], [14.25, 5.5, 1.0, 1.0], ("↓", None, None)),
        key([5, 15], [15.25, 5.5, 1.0, 1.0], ("→", None, None)),
        key([5, 16], [16.5, 5.25, 1.0, 1.0], ("Num 0", None, None)),
        key([5, 17], [17.5, 5.25, 1.0, 1.0], ("Num .", None, None)),
    ],
};

const fn key(
    matrix: [u8; 2],
    [x, y, width, height]: [f32; 4],
    (base, shift, alt_gr): (&'static str, Option<&'static str>, Option<&'static str>),
) -> PhysicalKey {
    PhysicalKey {
        matrix,
        bounds: Rectangle {
            x,
            y,
            width,
            height,
        },
        secondary_bounds: None,
        encoder: None,
        legends: Legends {
            base,
            shift,
            alt_gr,
        },
    }
}
