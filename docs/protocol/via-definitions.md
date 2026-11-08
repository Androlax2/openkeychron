# VIA definition files and the device registry

`openkeychron_via::ViaDefinition::from_json` reads a keyboard's physical layout from its VIA
definition file, the JSON Keychron publishes for each model under
`keyboards/keychron/<model>/via_json/` in its QMK repository. `DeviceRegistry` maps a
USB vendor and product ID to a layout: the built-in V5 Max FR ISO table, plus any
definition registered at run time.

No definition file is shipped with openkeychron: Keychron's files are GPL-2.0, and this
project is MIT OR Apache-2.0 (`docs/DECISIONS.md`, 2026-11-07). The user passes the file for
their keyboard:

```sh
openkeychron keymap show --layout v5_max_ansi_encoder.json
```

## What is read

Only `name`, `vendorId`, `productId` (hexadecimal strings such as `"0x3434"`),
`matrix.rows`, `matrix.cols` and `layouts.keymap`. Other top-level fields (`menus`,
`keycodes`, `customKeycodes`, `firmwareVersion`, ...) are ignored. Unknown fields inside
`matrix` or `layouts` are rejected.

`layouts.keymap` is in keyboard-layout-editor (KLE) form, read with the rules of VIA's reader,
[`kle-parser.ts`](https://github.com/the-via/reader/blob/c26009992a7a2e9776720ebf5644a50f8e92c7a5/src/kle-parser.ts):

- Each array is a row. A row starts at x = 0, one unit below the previous row's y.
- An object sets properties for the next key: `x` and `y` move the cursor, `w` and `h` set the
  size, `x2`, `y2`, `w2` and `h2` set a second rectangle relative to the key (ISO Enter).
  `w2` and `h2` default to `w` and `h`. Size and second rectangle reset after each key;
  `x` and `y` offsets accumulate.
- A string is a key. Its labels are split on `\n` and trimmed. Label 0 is the matrix position
  `row,column`; label 9 is an encoder `e<index>`, which marks the encoder's press.
- The layout is shifted so its top-left key edge is at (0, 0), as VIA draws it.

Colour (`c`, `t`), which VIA uses only for its key colour theme, and every other KLE property
VIA ignores are ignored.
Keys carry no legends: VIA labels name matrix positions, not keycaps.

## What is rejected

The loader fails with an error naming the item (`layouts.keymap[row][item]`) instead of
approximating:

| Input | Why |
|---|---|
| `r`, `rx`, `ry` | Rotated keys need geometry `Rectangle` cannot hold |
| `a` | Label alignment renumbers labels, so the matrix label could be misread |
| `d` | Decals are not keys; supporting them needs keys without a matrix position |
| A non-empty `layouts.labels`, or a key with label 3 | Layout options; only the default option exists today |
| A key with an encoder label but no matrix position | VIA allows encoders without a press; `DefinitionKey` needs a matrix position |
| A row of `layouts.keymap` that is not an array | VIA drops KLE metadata objects; none of Keychron's files has one |
| A matrix position outside `matrix`, or used twice | The keymap read would not match the matrix |
| A non-number geometry property, a bad `row,column` or encoder label | The file is malformed |

## Upstream coverage

Checked once against all 282 files under `keyboards/keychron/*/via_json/` at Keychron QMK
revision `2b493ff95f2665db719b1e95c91babb5036e72f3` (not committed, see above):

- 249 load.
- 31 use rotated keys (K11 Max, K15 Max, Q8, Q8 Max, Q10, Q10 Max, Q13 Max, Q14 Max, V8, V8 Max,
  V10, V10 Max) and are rejected.
- `k17_max_jis_knob_white.json` places key `1,19` outside its 6 × 19 matrix, and
  `k7_max_ansi_white.json` is empty; both are rejected.
- Loading `v5_max_iso_encoder.json` gives exactly the matrix positions, rectangles, second
  rectangles and encoder of the built-in `V5_MAX_FR_ISO` table, all 99 keys.

## Reading, and which models are written

The registry says which layout to draw, and each entry says on its own whether writes to that
model were verified on hardware (`WriteAccess`). `keymap show` uses it and still refuses a
receiver, which does not identify the keyboard behind it.

Only the built-in V5 Max ISO is `VerifiedOverCable`. A definition registered from a file is
always `ReadOnly`, and no write command accepts `--layout`, so a layout file never enables a
write. Keymap and macro writes, `rgb key` and the `rgb mixed` writes ask the built-in registry
for a write-verified model and refuse a receiver; keymap, macro and mixed RGB backups must name
a write-verified model.

A definition for a model the registry already knows is refused, so a file cannot replace the
built-in V5 Max ISO legends or its write access.

## Verification

```sh
cargo test -p openkeychron-via --locked --test definition
cargo test -p openkeychron-cli --locked --test keymap_definition
```

The tests use `fixtures/synthetic/via-definition-second-model.json`, a hand-written 2 × 4
definition that reuses the V5 Max ANSI ID (`3434:0950`) only to stand for a second model, and
`fixtures/synthetic/cli-keymap-second-model.toml`, its hand-written keymap replay.
