import { describe, expect, it } from "vitest";
import {
  BASIC_CATEGORIES,
  buildKeycode,
  keycodeName,
  type TickedModifiers,
  wrappedLayers,
} from "./keycode-build";

const NONE: TickedModifiers = {
  ctrl: false,
  shift: false,
  alt: false,
  gui: false,
  rightHand: false,
};
const KC_A = 0x04;
const KC_C = 0x06;
const KC_SPC = 0x2c;

describe("buildKeycode", () => {
  it("builds a modifier combination from its modifiers and key", () => {
    expect(
      buildKeycode("modified", { modifiers: { ...NONE, ctrl: true }, layer: null, key: KC_C }),
    ).toBe(0x0106);
  });

  it("builds a right-hand modifier combination", () => {
    const modifiers = { ...NONE, ctrl: true, shift: true, rightHand: true };
    expect(buildKeycode("modified", { modifiers, layer: null, key: KC_A })).toBe(0x1304);
  });

  it("builds a mod-tap from its modifiers and tap key", () => {
    expect(
      buildKeycode("modTap", { modifiers: { ...NONE, shift: true }, layer: null, key: KC_A }),
    ).toBe(0x2204);
  });

  it("builds a layer-tap from its layer and tap key", () => {
    expect(buildKeycode("layerTap", { modifiers: NONE, layer: 1, key: KC_SPC })).toBe(0x412c);
  });

  it("builds a layer-mod from its layer and modifiers", () => {
    expect(
      buildKeycode("layerMod", { modifiers: { ...NONE, ctrl: true }, layer: 1, key: null }),
    ).toBe(0x5021);
  });

  it("builds nothing without a modifier", () => {
    expect(
      buildKeycode("modified", {
        modifiers: { ...NONE, rightHand: true },
        layer: null,
        key: KC_C,
      }),
    ).toBeNull();
  });

  it("builds nothing without a key", () => {
    expect(
      buildKeycode("modTap", { modifiers: { ...NONE, ctrl: true }, layer: null, key: null }),
    ).toBeNull();
  });

  it("builds nothing without a layer", () => {
    expect(buildKeycode("layerTap", { modifiers: NONE, layer: null, key: KC_SPC })).toBeNull();
  });

  it("builds nothing on a layer a two-part keycode cannot hold", () => {
    expect(buildKeycode("layerTap", { modifiers: NONE, layer: 16, key: KC_SPC })).toBeNull();
  });
});

describe("keycodeName", () => {
  it.each([
    [0x0029, "ESC"],
    [0x5221, "MO(1)"],
    [0x0106, "LCTL(C)"],
    [0x1304, "RCTL(RSFT(A))"],
    [0x2204, "LSFT_T(A)"],
    [0x2304, "MT(MOD_LCTL | MOD_LSFT, A)"],
    [0x412c, "LT(1, SPC)"],
    [0x5021, "LM(1, MOD_LCTL)"],
    [0x52a2, "OSM(MOD_LSFT)"],
    [0x7c00, "QK_BOOT"],
  ])("names %i as %s", (keycode, name) => {
    expect(keycodeName(keycode)).toBe(name);
  });

  it("gives no name to a keycode QMK does not name", () => {
    expect(keycodeName(0x7e05)).toBeNull();
  });
});

describe("BASIC_CATEGORIES", () => {
  it("offers no layer keycode as a key to wrap", () => {
    expect(BASIC_CATEGORIES.flatMap((category) => category.keycodes)).not.toContainEqual(
      expect.objectContaining({ name: "MO(1)" }),
    );
  });

  it("keeps the basic keycodes", () => {
    expect(BASIC_CATEGORIES.map((category) => category.name)).toContain("Letters");
  });
});

describe("wrappedLayers", () => {
  it("lists the 16 layers a layer-tap holds", () => {
    expect(wrappedLayers("layerTap")).toEqual([...Array(16).keys()]);
  });

  it("lists no layer for a modifier combination", () => {
    expect(wrappedLayers("modified")).toEqual([]);
  });
});
