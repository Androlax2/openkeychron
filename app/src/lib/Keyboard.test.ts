import { flushSync, mount, unmount } from "svelte";
import { afterEach, describe, expect, it } from "vitest";
import { KEYMAP } from "../test/synthetic-keymap";
import type { Keymap } from "./bindings";
import Keyboard, { KEY_UNIT } from "./Keyboard.svelte";

let mounted: Record<string, unknown> | null = null;

function render(keymap: Keymap = KEYMAP): HTMLElement {
  const target = document.createElement("div");
  document.body.append(target);
  mounted = mount(Keyboard, { target, props: { keymap } });
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

  it("shows the keycode the shown layer assigns to each key", () => {
    const view = render();
    expect(keyNamed(view, "A").querySelector("text.keycode")?.textContent).toBe("0x0009");
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
      "Selected: é (row 0, column 1), keycode 0x0005",
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
    expect(keyNamed(view, "A").querySelector("text.keycode")?.textContent).toBe("0x0105");
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
});
