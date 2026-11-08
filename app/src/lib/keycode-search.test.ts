import { describe, expect, it } from "vitest";
import type { KeycodeCategory } from "./bindings";
import { matchingCategories } from "./keycode-search";

// Hand-written, cut down from KEYCODE_CATEGORIES.
const CATEGORIES: KeycodeCategory[] = [
  {
    name: "Letters",
    keycodes: [
      { keycode: 0x0004, name: "A", layer: null },
      { keycode: 0x0005, name: "B", layer: null },
    ],
  },
  {
    name: "Editing",
    keycodes: [
      { keycode: 0x0029, name: "ESC", layer: null },
      { keycode: 0x002a, name: "BSPC", layer: null },
    ],
  },
  {
    name: "Layers",
    keycodes: [
      { keycode: 0x5220, name: "MO(0)", layer: 0 },
      { keycode: 0x5221, name: "MO(1)", layer: 1 },
      { keycode: 0x5222, name: "MO(2)", layer: 2 },
    ],
  },
];

function names(categories: readonly KeycodeCategory[]): string[][] {
  return categories.map((category) => [
    category.name,
    ...category.keycodes.map((choice) => choice.name),
  ]);
}

describe("matchingCategories", () => {
  it("offers every keycode of the keyboard's layers when the search is empty", () => {
    expect(names(matchingCategories(CATEGORIES, "", 2))).toEqual([
      ["Letters", "A", "B"],
      ["Editing", "ESC", "BSPC"],
      ["Layers", "MO(0)", "MO(1)"],
    ]);
  });

  it("keeps the keycodes whose name contains the search, ignoring case", () => {
    expect(names(matchingCategories(CATEGORIES, "bsp", 2))).toEqual([["Editing", "BSPC"]]);
  });

  it("keeps a whole category whose name contains the search", () => {
    expect(names(matchingCategories(CATEGORIES, "letter", 2))).toEqual([["Letters", "A", "B"]]);
  });

  it("finds a keycode by its hexadecimal value", () => {
    expect(names(matchingCategories(CATEGORIES, "0x0029", 2))).toEqual([["Editing", "ESC"]]);
  });

  it("ignores spaces around the search", () => {
    expect(names(matchingCategories(CATEGORIES, "  esc ", 2))).toEqual([["Editing", "ESC"]]);
  });

  it("offers no layer keycode for a layer the keyboard does not have", () => {
    expect(names(matchingCategories(CATEGORIES, "mo(2)", 2))).toEqual([]);
  });
});
