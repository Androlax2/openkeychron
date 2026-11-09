import { flushSync, mount, unmount } from "svelte";
import { afterEach, describe, expect, it } from "vitest";
import { KEYMAP, KNOB_KEYMAP, withKeycode, withRotation } from "../test/synthetic-keymap";
import type { Keymap } from "./bindings";
import Keyboard, { KEY_UNIT, type KeySelection } from "./Keyboard.svelte";

let mounted: Record<string, unknown> | null = null;

function render(
  keymap: Keymap = KEYMAP,
  onselectionchange: (selection: KeySelection) => void = () => {},
): HTMLElement {
  const target = document.createElement("div");
  document.body.append(target);
  mounted = mount(Keyboard, { target, props: { keymap, onselectionchange } });
  flushSync();
  return target;
}

afterEach(() => {
  if (mounted !== null) {
    unmount(mounted);
    mounted = null;
  }
  document.body.replaceChildren();
});

function keyElements(view: HTMLElement): SVGGElement[] {
  return [...view.querySelectorAll<SVGGElement>('[role="option"]')];
}

/** The base legend starts the key's accessible name, as in "é, Shift 2, AltGr ~, …". */
function baseLegendOf(key: SVGGElement): string | undefined {
  return key.getAttribute("aria-label")?.split(", ")[0];
}

function keyNamed(view: HTMLElement, legend: string): SVGGElement {
  const found = keyElements(view).find((key) => baseLegendOf(key) === legend);
  if (found === undefined) {
    throw new Error(`no key with the legend ${legend}`);
  }
  return found;
}

function press(element: Element, key: string): void {
  element.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
  flushSync();
}

function click(element: Element): void {
  element.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  flushSync();
}

function selectedLegends(view: HTMLElement): (string | undefined)[] {
  return keyElements(view)
    .filter((key) => key.getAttribute("aria-selected") === "true")
    .map((key) => baseLegendOf(key));
}

function layerTabs(view: HTMLElement): HTMLButtonElement[] {
  return [...view.querySelectorAll<HTMLButtonElement>('[role="tab"]')];
}

describe("Keyboard", () => {
  it("draws one key for each physical matrix slot, not for unused ones", () => {
    const view = render();
    expect(keyElements(view).map((key) => baseLegendOf(key))).toEqual([
      "&",
      "é",
      '"',
      "Num Lock",
      "A",
      "Z",
      "E",
      "Enter",
    ]);
  });

  it("places each key at its layout position, in key units", () => {
    const view = render();
    expect(keyNamed(view, "A").getAttribute("transform")).toBe(
      `translate(${1.5 * KEY_UNIT} ${2.25 * KEY_UNIT})`,
    );
  });

  it("sizes the drawing to the keys' far right and bottom edges", () => {
    const view = render();
    expect(view.querySelector("svg")?.getAttribute("viewBox")).toBe(
      `0 0 ${16.5 * KEY_UNIT} ${4.25 * KEY_UNIT}`,
    );
  });

  it("leaves a key without a rotation unturned", () => {
    const view = render();
    expect(keyNamed(view, "A").getAttribute("transform")).not.toContain("rotate");
  });

  it("turns a rotated key about its origin, after placing it", () => {
    const keymap = withRotation(
      KEYMAP,
      { row: 1, column: 0 },
      {
        angleDegrees: 15,
        originX: 2,
        originY: 3,
      },
    );
    const view = render(keymap);
    expect(keyNamed(view, "A").getAttribute("transform")).toBe(
      `rotate(15 ${2 * KEY_UNIT} ${3 * KEY_UNIT}) translate(${1.5 * KEY_UNIT} ${2.25 * KEY_UNIT})`,
    );
  });

  it("turns a rotated ISO Enter as one key, so both of its rectangles stay joined", () => {
    const keymap = withRotation(
      KEYMAP,
      { row: 1, column: 3 },
      {
        angleDegrees: 90,
        originX: 13.5,
        originY: 2.25,
      },
    );
    const enter = keyNamed(render(keymap), "Enter");
    expect(enter.getAttribute("transform")).toBe(
      `rotate(90 ${13.5 * KEY_UNIT} ${2.25 * KEY_UNIT}) translate(${13.75 * KEY_UNIT} ${2.25 * KEY_UNIT})`,
    );
    expect(enter.querySelectorAll("rect.cap-fill")).toHaveLength(2);
  });

  it("sizes the drawing to the far edges of rotated keys", () => {
    const keymap = withRotation(
      KEYMAP,
      { row: 0, column: 4 },
      {
        angleDegrees: 45,
        originX: 15.5,
        originY: 1.25,
      },
    );
    const viewBox = render(keymap).querySelector("svg")?.getAttribute("viewBox") ?? "";
    const [, , drawnWidth, drawnHeight] = viewBox.split(" ").map(Number);
    expect(drawnWidth).toBeCloseTo((15.5 + Math.SQRT1_2) * KEY_UNIT);
    expect(drawnHeight).toBeCloseTo(4.25 * KEY_UNIT);
  });

  it("draws the ISO Enter from both of its rectangles, relative to the key", () => {
    const view = render();
    const fills = [...keyNamed(view, "Enter").querySelectorAll("rect.cap-fill")];
    const gap = 3;
    expect(
      fills.map((fill) =>
        ["x", "y", "width", "height"].map((attribute) => Number(fill.getAttribute(attribute))),
      ),
    ).toEqual([
      [gap, gap, 1.25 * KEY_UNIT - 2 * gap, 2 * KEY_UNIT - 2 * gap],
      [-0.25 * KEY_UNIT + gap, gap, 1.5 * KEY_UNIT - 2 * gap, KEY_UNIT - 2 * gap],
    ]);
  });

  it("breaks a long legend onto two lines on a 1u key", () => {
    const view = render();
    const lines = [...keyNamed(view, "Num Lock").querySelectorAll("text.base tspan")];
    expect(lines.map((line) => line.textContent)).toEqual(["Num", "Lock"]);
  });

  it("shows the French base, Shift and AltGr legends on the keycap", () => {
    const view = render();
    const texts = [...keyNamed(view, "é").querySelectorAll("text")].map((text) => text.textContent);
    expect(texts).toEqual(expect.arrayContaining(["é", "2", "~"]));
  });

  it("shows what a French AZERTY host types for the keycode the shown layer assigns to a key", () => {
    const view = render();
    expect(keyNamed(view, "A").querySelector("text.keycode")?.textContent?.trim()).toBe("a");
  });

  it("shows the QMK name of a keycode the French AZERTY layout does not cover", () => {
    const view = render();
    expect(keyNamed(view, "Enter").querySelector("text.keycode")?.textContent?.trim()).toBe("ENT");
  });

  it("shows a keycode without a name as its raw hexadecimal value", () => {
    const view = render();
    expect(keyNamed(view, "E").querySelector("text.keycode")?.textContent?.trim()).toBe("0x7e00");
  });

  it("gives the keycode's name and raw value in the key's accessible name", () => {
    const view = render();
    expect(keyNamed(view, "A").getAttribute("aria-label")).toBe("A, types a, keycode Q (0x0014)");
  });

  it("leaves the typed character out of the accessible name of a keycode AZERTY does not cover", () => {
    const view = render();
    expect(keyNamed(view, "Enter").getAttribute("aria-label")).toBe("Enter, keycode ENT (0x0028)");
  });

  it("leaves a keycode name that fits its keycap at its normal size", () => {
    const view = render();
    expect(keyNamed(view, "A").querySelector<SVGTextElement>("text.keycode")?.style.fontSize).toBe(
      "",
    );
  });

  it("selects a key when it is clicked", () => {
    const view = render();
    click(keyNamed(view, "Z"));
    expect(selectedLegends(view)).toEqual(["Z"]);
  });

  it("names the selected key and the keycode the shown layer gives it", () => {
    const view = render();
    click(keyNamed(view, "é"));
    expect(view.querySelector(".selection")?.textContent?.trim()).toBe(
      "Selected: é (row 0, column 1), keycode 2 (0x001f)",
    );
  });

  it("keeps QMK's name visible for the selected key that shows what it types", () => {
    const view = render();
    click(keyNamed(view, "A"));
    expect(view.querySelector(".selection")?.textContent?.trim()).toBe(
      "Selected: A (row 1, column 0), keycode Q (0x0014)",
    );
  });

  it("gives only the raw value of a selected key's keycode without a name", () => {
    const view = render();
    click(keyNamed(view, "E"));
    expect(view.querySelector(".selection")?.textContent?.trim()).toBe(
      "Selected: E (row 1, column 2), keycode 0x7e00",
    );
  });

  it("moves the selection when another key is clicked", () => {
    const view = render();
    click(keyNamed(view, "Z"));
    click(keyNamed(view, "Enter"));
    expect(selectedLegends(view)).toEqual(["Enter"]);
  });

  it.each(["Enter", " "])("selects the focused key with the %j key", (keyName) => {
    const view = render();
    press(keyNamed(view, "E"), keyName);
    expect(selectedLegends(view)).toEqual(["E"]);
  });

  it("puts only one key in the tab order", () => {
    const view = render();
    const tabbable = keyElements(view).filter((key) => key.getAttribute("tabindex") === "0");
    expect(tabbable.map((key) => baseLegendOf(key))).toEqual(["&"]);
  });

  it.each([
    ["ArrowRight", "é", '"'],
    ["ArrowLeft", "é", "&"],
    ["ArrowDown", "é", "A"],
    ["ArrowDown", '"', "Z"],
    ["ArrowUp", "A", "&"],
    ["ArrowRight", "E", "Enter"],
    ["ArrowLeft", "&", "&"],
  ])("moves focus with %s from %s to %s", (arrow, from, to) => {
    const view = render();
    const start = keyNamed(view, from);
    start.focus();
    press(start, arrow);
    expect(baseLegendOf(document.activeElement as SVGGElement)).toBe(to);
    expect(keyNamed(view, to).getAttribute("tabindex")).toBe("0");
  });

  it("does not select a key when focus only moves to it", () => {
    const view = render();
    const start = keyNamed(view, "é");
    start.focus();
    press(start, "ArrowRight");
    expect(selectedLegends(view)).toEqual([]);
  });

  it("offers one tab per layer, the first one shown", () => {
    const view = render();
    expect(
      layerTabs(view).map((tab) => [tab.textContent?.trim(), tab.getAttribute("aria-selected")]),
    ).toEqual([
      ["Layer 0", "true"],
      ["Layer 1", "false"],
    ]);
  });

  it("shows the other layer's keycodes when its tab is clicked", () => {
    const view = render();
    const [, secondLayer] = layerTabs(view);
    if (secondLayer === undefined) {
      throw new Error("no second layer tab");
    }
    click(secondLayer);
    expect(keyNamed(view, "A").querySelector("text.keycode")?.textContent?.trim()).toBe(
      "LT(1, SPC)",
    );
  });

  it("shrinks a keycode name wider than its keycap to fit the keycap", () => {
    const view = render();
    const [, secondLayer] = layerTabs(view);
    if (secondLayer === undefined) {
      throw new Error("no second layer tab");
    }
    click(secondLayer);
    const keycode = keyNamed(view, "A").querySelector<SVGTextElement>("text.keycode");
    // "LT(1, SPC)": an estimated 10 × 6px in the 40px between the 1u key's 10px margins.
    expect(keycode?.style.fontSize).toBe(`${(9 * 40) / 60}px`);
  });

  describe("a 26-character name on a 1u key", () => {
    const MOD_TAP_NAME = "MT(MOD_LCTL | MOD_LGUI, A)";
    const withLongName = (name: string) =>
      withKeycode(KEYMAP, { layer: 0, row: 1, column: 0 }, 0x2304, name);

    it("is cut with an ellipsis at a readable size", () => {
      const view = render(withLongName(MOD_TAP_NAME));
      const keycode = keyNamed(view, "A").querySelector<SVGTextElement>("text.keycode");
      expect(keycode?.textContent?.trim().endsWith("…")).toBe(true);
      expect(keycode?.style.fontSize).toBe("6px");
    });

    it("keeps the full name in the hover title and the accessible name", () => {
      const view = render(withLongName(MOD_TAP_NAME));
      const key = keyNamed(view, "A");
      expect(key.querySelector("title")?.textContent).toBe(`${MOD_TAP_NAME} (0x2304)`);
      expect(key.getAttribute("aria-label")).toContain(MOD_TAP_NAME);
    });

    it("is written as QMK's shorthand when QMK has one", () => {
      const view = render(withLongName("LCTL(LSFT(LALT(LGUI(A))))"));
      const keycode = keyNamed(view, "A").querySelector<SVGTextElement>("text.keycode");
      expect(keycode?.textContent?.trim()).toBe("HYPR(A)");
    });
  });

  it("moves between layer tabs with the arrow keys", () => {
    const view = render();
    const [firstLayer] = layerTabs(view);
    if (firstLayer === undefined) {
      throw new Error("no first layer tab");
    }
    firstLayer.focus();
    press(firstLayer, "ArrowRight");
    expect(document.activeElement?.textContent?.trim()).toBe("Layer 1");
    expect(layerTabs(view)[1]?.getAttribute("aria-selected")).toBe("true");
  });

  it("keeps the selected key when the layer changes", () => {
    const view = render();
    click(keyNamed(view, "Z"));
    const [, secondLayer] = layerTabs(view);
    if (secondLayer === undefined) {
      throw new Error("no second layer tab");
    }
    click(secondLayer);
    expect(selectedLegends(view)).toEqual(["Z"]);
  });

  it("reports the selected key with the index of the layer it is shown on", () => {
    const selections: KeySelection[] = [];
    const view = render(KEYMAP, (selection) => selections.push(selection));
    click(keyNamed(view, "Z"));
    expect(selections).toEqual([{ layer: 0, row: 1, column: 1 }]);
  });

  it("reports the selected key again on the layer the user switches to", () => {
    const selections: KeySelection[] = [];
    const view = render(KEYMAP, (selection) => selections.push(selection));
    click(keyNamed(view, "Z"));
    const [, secondLayer] = layerTabs(view);
    if (secondLayer === undefined) {
      throw new Error("no second layer tab");
    }
    click(secondLayer);
    expect(selections.at(-1)).toEqual({ layer: 1, row: 1, column: 1 });
  });

  it("reports no selection when the layer changes before a key is selected", () => {
    const selections: KeySelection[] = [];
    const view = render(KEYMAP, (selection) => selections.push(selection));
    const [, secondLayer] = layerTabs(view);
    if (secondLayer === undefined) {
      throw new Error("no second layer tab");
    }
    click(secondLayer);
    expect(selections).toEqual([]);
  });

  describe("the knob's turns", () => {
    function knobKey(view: HTMLElement): SVGGElement {
      return keyNamed(view, "Knob");
    }

    function turnTexts(key: SVGGElement): string[] {
      return [...key.querySelectorAll(".turn")].map((turn) => turn.textContent?.trim() ?? "");
    }

    it("names what each turn sends in the knob key's accessible name", () => {
      const label = knobKey(render(KNOB_KEYMAP)).getAttribute("aria-label");
      expect(label).toContain("turned counter-clockwise sends VOLD (0x00aa)");
      expect(label).toContain("turned clockwise sends VOLU (0x00a9)");
    });

    it("draws what each turn sends on the knob key", () => {
      expect(turnTexts(knobKey(render(KNOB_KEYMAP)))).toEqual(["↺ VOLD", "↻ VOLU"]);
    });

    it("shows the turns of the layer being shown", () => {
      const view = render(KNOB_KEYMAP);
      const [, secondLayer] = layerTabs(view);
      if (secondLayer === undefined) {
        throw new Error("no second layer tab");
      }
      click(secondLayer);
      expect(turnTexts(knobKey(view))).toEqual(["↺ TRNS", "↻ TRNS"]);
    });

    it("shows the raw value of a turn whose keycode has no name", () => {
      const keymap: Keymap = {
        ...KNOB_KEYMAP,
        layers: KNOB_KEYMAP.layers.map((layer) => ({
          ...layer,
          encoders: layer.encoders.map((encoder) => ({
            ...encoder,
            clockwise: { keycode: 0x7e40, name: null },
          })),
        })),
      };
      expect(turnTexts(knobKey(render(keymap)))).toEqual(["↺ VOLD", "↻ 0x7e40"]);
    });

    it("draws no turns on a key that is not an encoder's press", () => {
      expect(turnTexts(keyNamed(render(KNOB_KEYMAP), "Z"))).toEqual([]);
    });

    it("names no turns in the accessible name of an ordinary key", () => {
      expect(keyNamed(render(KNOB_KEYMAP), "Z").getAttribute("aria-label")).not.toContain("turned");
    });

    it("shrinks a turn label too long for the key", () => {
      const keymap: Keymap = {
        ...KNOB_KEYMAP,
        layers: KNOB_KEYMAP.layers.map((layer) => ({
          ...layer,
          encoders: layer.encoders.map((encoder) => ({
            ...encoder,
            clockwise: { keycode: 0x00ee, name: "KC_MS_WH_UP" },
          })),
        })),
      };
      const turn = knobKey(render(keymap)).querySelectorAll<SVGTextElement>(".turn")[1];
      expect(turn?.style.fontSize).not.toBe("");
    });

    it("draws no turns when the layer has no entry for the key's encoder", () => {
      const keymap: Keymap = {
        ...KNOB_KEYMAP,
        layers: KNOB_KEYMAP.layers.map((layer) => ({ ...layer, encoders: [] })),
      };
      const key = knobKey(render(keymap));
      expect(turnTexts(key)).toEqual([]);
      expect(key.getAttribute("aria-label")).not.toContain("turned");
    });
  });
});
