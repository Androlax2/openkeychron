// Test data only.
import type { Bounds, Key, Keymap } from "../lib/bindings";

// A hand-written, cut-down V5 Max FR ISO keymap: two rows of real positions, the ISO Enter,
// the 1u Num Lock and two unused matrix slots. Bounds come from the V5 Max ISO layout in openkeychron-via.
const COLUMNS = 5;

function physicalKey(
  row: number,
  column: number,
  legends: [string, string | null, string | null],
  bounds: Bounds,
  secondaryBounds: Bounds | null = null,
): (keycode: number) => Key {
  const [legend, shiftLegend, altGrLegend] = legends;
  return (keycode) => ({
    row,
    column,
    legend,
    shiftLegend,
    altGrLegend,
    isPhysical: true,
    bounds,
    secondaryBounds,
    keycode,
  });
}

const unusedSlot =
  (row: number, column: number) =>
  (keycode: number): Key => ({
    row,
    column,
    legend: null,
    shiftLegend: null,
    altGrLegend: null,
    isPhysical: false,
    bounds: null,
    secondaryBounds: null,
    keycode,
  });

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

export const KEYMAP: Keymap = {
  layerCount: 2,
  rows: 2,
  columns: COLUMNS,
  layers: [
    { index: 0, keys: KEYS.map((makeKey, slot) => makeKey(0x0004 + slot)) },
    { index: 1, keys: KEYS.map((makeKey, slot) => makeKey(0x0100 + slot)) },
  ],
};
