# V5 Max FR ISO physical layout

`openkeychron_via::V5_MAX_FR_ISO` describes the Keychron V5 Max ISO knob model
(`3434:0951`). It is a static, dependency-free table, not a device query or a default
keymap to write to the keyboard.

## Matrix and geometry

The matrix is **6 rows × 19 columns**, with **99 physical keys** and 15 unused cells.
`layout.key(row, column)` returns `None` for unused or out-of-range cells. The encoder's
press is the physical key at `(0,18)`, with `encoder: Some(0)`. Encoder rotation maps
are separate; this table does not invent physical keys for them.

Rectangles use keyboard units, with the origin at the top-left and y increasing downwards.
The table follows the VIA definition's physical order, **not flattened matrix order**;
for example, the numpad plus key `(3,18)` begins alongside matrix row 2.
Consumers must join keymap entries by `(row,column)`, not zip the two collections.
Reading a keymap still requires `layers × 6 × 19` entries, not `layers × 99`.

The VIA definition uses KLE geometry. Relative x/y offsets accumulate; widths/heights
reset after each key, and each new physical row starts at x = 0 and advances y by 1.
Both `bounds` and `secondary_bounds` are **absolute layout coordinates**. ISO Enter at
`(2,13)` is the union of `(13.75,2.25,1.25,2)` and `(13.5,2.25,1.5,1)`, preserving its
non-rectangular outline rather than drawing a filled bounding box.

## Display legends only

French letters and base/Shift/AltGr symbols describe the conventional French AZERTY
layout. Letters are uppercase key labels; a missing Shift legend does not mean Shift
has no effect. `None` means that no separate legend is provided, not a disabled key.
Accent labels such as `^`, `¨`, `~` and the grave accent may represent dead keys.
These strings are not an input method or a Unicode-output guarantee.

Non-character labels use the upstream Windows base key roles, with right Alt labelled
`AltGr`; the knob is labelled `Knob` rather than promising a particular press action.
Numpad labels identify the keys, not their output under Num Lock or a host locale.
Mac-mode roles, replacement keycaps, host locale and user remaps can differ.
No layer assignment, modifier state or active host layout is inferred from this table.
In particular, displaying `A` never changes the raw keycode associated with that position.

## Sources and licence note

All source references are pinned to Keychron QMK revision
`2b493ff95f2665db719b1e95c91babb5036e72f3`:

- [V5 Max ISO VIA JSON](https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/keyboards/keychron/v5_max/via_json/v5_max_iso_encoder.json): identity, matrix coordinates and geometry. It has no per-file licence notice; the upstream repository includes the [GNU GPL version 2](https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/LICENSE).
- [French aliases](https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/keymap_extras/keymap_french.h): copyright 2025 QMK, explicitly **GPL-2.0-or-later**.
- [Default ISO keymap](https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/keyboards/keychron/v5_max/iso_encoder/keymaps/keychron/keymap.c): copyright 2024–2025 Keychron, explicitly **GPL-2.0-or-later**; consulted for non-character key roles.

The Rust table restates device geometry and legend facts; it does not embed the upstream
JSON, C declarations, macros or firmware implementation. These references document the
upstream sources and their notices, not a claim that upstream files have been relicensed.
The project's existing licence files and Cargo licence metadata are unchanged.

## Verification

`cargo test -p openkeychron-via --locked layout` tests physical coverage, unused cells,
lookup bounds, ISO Enter geometry, wide/tall/offset keys, encoder annotation and French
legends through the public API. Expected values are literal and checked against the
sources above, not generated from the production table.

No real-device keymap capture or visual keycap verification is claimed. Cable-only
configuration and receiver-write refusal are unchanged; this table performs no HID I/O.
