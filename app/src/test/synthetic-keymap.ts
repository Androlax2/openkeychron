// Test data only.
import type { Bounds, EditableKeymap, Encoder, Key, Keymap } from "../lib/bindings";

// A hand-written, cut-down V5 Max FR ISO keymap: two rows of real positions, the ISO Enter,
// the 1u Num Lock and two unused matrix slots. Bounds come from the V5 Max ISO layout in openkeychron-via.
// Names are what openkeychron-via gives each keycode; a custom keycode has none.
const COLUMNS = 5;

type Assignment = [keycode: number, name: string | null];

// What a French AZERTY host types for the basic keycodes used below, as openkeychron-via gives it.
const AZERTY_TEXT = new Map<number, string>([
  [0x001e, "&"],
  [0x001f, "é"],
  [0x0020, '"'],
  [0x0014, "a"],
  [0x001a, "z"],
]);

function physicalKey(
  row: number,
  column: number,
  legends: [string, string | null, string | null],
  bounds: Bounds,
  secondaryBounds: Bounds | null = null,
): (assignment: Assignment) => Key {
  const [legend, shiftLegend, altGrLegend] = legends;
  return ([keycode, name]) => ({
    row,
    column,
    legend,
    shiftLegend,
    altGrLegend,
    isPhysical: true,
    bounds,
    secondaryBounds,
    keycode,
    name,
    azertyText: AZERTY_TEXT.get(keycode) ?? null,
    encoder: null,
  });
}

const unusedSlot =
  (row: number, column: number) =>
  ([keycode, name]: Assignment): Key => ({
    row,
    column,
    legend: null,
    shiftLegend: null,
    altGrLegend: null,
    isPhysical: false,
    bounds: null,
    secondaryBounds: null,
    keycode,
    name,
    azertyText: null,
    encoder: null,
  });

const NO: Assignment = [0x0000, "NO"];
const TRNS: Assignment = [0x0001, "TRNS"];

const KEYS = [
  physicalKey(0, 0, ["&", "1", null], { x: 1, y: 1.25, width: 1, height: 1 }),
  physicalKey(0, 1, ["é", "2", "~"], { x: 2, y: 1.25, width: 1, height: 1 }),
  physicalKey(0, 2, ['"', "3", "#"], { x: 3, y: 1.25, width: 1, height: 1 }),
  unusedSlot(0, 3),
  physicalKey(0, 4, ["Num Lock", null, null], { x: 15.5, y: 1.25, width: 1, height: 1 }),
  physicalKey(1, 0, ["A", null, null], { x: 1.5, y: 2.25, width: 1, height: 1 }),
  physicalKey(1, 1, ["Z", null, null], { x: 2.5, y: 2.25, width: 1, height: 1 }),
  physicalKey(1, 2, ["E", null, "€"], { x: 3.5, y: 2.25, width: 1, height: 1 }),
  physicalKey(
    1,
    3,
    ["Enter", null, null],
    { x: 13.75, y: 2.25, width: 1.25, height: 2 },
    { x: 13.5, y: 2.25, width: 1.5, height: 1 },
  ),
  unusedSlot(1, 4),
];

// In matrix order, one per entry of KEYS.
const FRENCH_TYPING: Assignment[] = [
  [0x001e, "1"],
  [0x001f, "2"],
  [0x0020, "3"],
  NO,
  [0x0053, "NUM"],
  [0x0014, "Q"],
  [0x001a, "W"],
  [0x7e00, null],
  [0x0028, "ENT"],
  NO,
];

const FUNCTION_LAYER: Assignment[] = [
  [0x5221, "MO(1)"],
  [0x0106, "LCTL(C)"],
  TRNS,
  NO,
  TRNS,
  [0x412c, "LT(1, SPC)"],
  TRNS,
  [0xffff, null],
  TRNS,
  NO,
];

/** The V5 Max's one encoder, which the cut-down layout has no key for. */
function knob(counterClockwise: Assignment, clockwise: Assignment): Encoder {
  return {
    index: 0,
    counterClockwise: { keycode: counterClockwise[0], name: counterClockwise[1] },
    clockwise: { keycode: clockwise[0], name: clockwise[1] },
  };
}

function assign(assignments: Assignment[]): Key[] {
  return KEYS.map((makeKey, slot) => {
    const assignment = assignments[slot];
    if (assignment === undefined) {
      throw new Error(`no assignment for matrix slot ${slot}`);
    }
    return makeKey(assignment);
  });
}

export const KEYMAP: Keymap = {
  layerCount: 2,
  rows: 2,
  columns: COLUMNS,
  layers: [
    { index: 0, keys: assign(FRENCH_TYPING), encoders: [knob([0x00aa, "VOLD"], [0x00a9, "VOLU"])] },
    { index: 1, keys: assign(FUNCTION_LAYER), encoders: [knob(TRNS, TRNS)] },
  ],
  // A partial list of the V5 Max ISO names; 0x7e05 (LNPAD there) is left out on purpose, to test a keycode without a name.
  customKeycodeNames: [
    { keycode: 0x7e00, name: "LOPTN" },
    { keycode: 0x7e08, name: "SNAP" },
  ],
};

/** KEYMAP as `read_keymap` returns it before the app changed anything. */
export const EDITABLE_KEYMAP: EditableKeymap = {
  keymap: KEYMAP,
  undoSteps: 0,
  redoSteps: 0,
  canRestoreBackup: false,
};

/**
 * KEYMAP with Num Lock's slot made the press of encoder 0: the cut-down layout has no room for
 * the V5 Max's knob at row 0, column 18.
 */
export const KNOB = { layer: 0, row: 0, column: 4 };

export const KNOB_KEYMAP: Keymap = {
  ...KEYMAP,
  layers: KEYMAP.layers.map((layer) => ({
    ...layer,
    keys: layer.keys.map((key) =>
      key.row === KNOB.row && key.column === KNOB.column
        ? { ...key, legend: "Knob", encoder: 0 }
        : key,
    ),
  })),
};

export const EDITABLE_KNOB: EditableKeymap = { ...EDITABLE_KEYMAP, keymap: KNOB_KEYMAP };

/** `keymap` with the key at `row`, `column` on layer `layer` given `keycode` and `name`. */
export function withKeycode(
  keymap: Keymap,
  position: { layer: number; row: number; column: number },
  keycode: number,
  name: string | null,
): Keymap {
  return {
    ...keymap,
    layers: keymap.layers.map((layer) =>
      layer.index !== position.layer
        ? layer
        : {
            ...layer,
            keys: layer.keys.map((key) =>
              key.row === position.row && key.column === position.column
                ? { ...key, keycode, name, azertyText: AZERTY_TEXT.get(keycode) ?? null }
                : key,
            ),
          },
    ),
  };
}
