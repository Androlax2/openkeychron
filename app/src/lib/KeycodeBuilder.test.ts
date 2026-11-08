import { flushSync, mount, unmount } from "svelte";
import { afterEach, describe, expect, it } from "vitest";
import type { WrapperBuilder } from "./bindings";
import KeycodeBuilder from "./KeycodeBuilder.svelte";

let mounted: Record<string, unknown> | null = null;

afterEach(() => {
  if (mounted !== null) {
    unmount(mounted);
    mounted = null;
  }
  document.body.replaceChildren();
});

type View = { element: HTMLElement; built: (number | null)[] };

function render(builder: WrapperBuilder, layerCount = 4): View {
  const target = document.createElement("div");
  document.body.append(target);
  const built: (number | null)[] = [];
  mounted = mount(KeycodeBuilder, {
    target,
    props: { builder, layerCount, onbuild: (keycode: number | null) => built.push(keycode) },
  });
  flushSync();
  return { element: target, built };
}

function control(view: View, text: string): HTMLInputElement {
  const label = [...view.element.querySelectorAll("label")].find(
    (candidate) => candidate.textContent?.trim() === text,
  );
  const input = label?.querySelector("input");
  if (input === null || input === undefined) {
    throw new Error(`no control labelled ${text}`);
  }
  return input;
}

function click(view: View, text: string): void {
  control(view, text).click();
  flushSync();
}

function layerSelect(view: View): HTMLSelectElement {
  const select = view.element.querySelector<HTMLSelectElement>("select");
  if (select === null) {
    throw new Error("no layer select");
  }
  return select;
}

function chooseLayer(view: View, layer: number): void {
  const select = layerSelect(view);
  select.value = String(layer);
  select.dispatchEvent(new Event("change", { bubbles: true }));
  flushSync();
}

function missing(view: View): string | undefined {
  return view.element.querySelector(".missing")?.textContent?.trim();
}

function lastBuilt(view: View): number | null | undefined {
  return view.built.at(-1);
}

describe("KeycodeBuilder", () => {
  it("builds a modifier combination from ticked modifiers and a key", () => {
    const view = render("modified");
    click(view, "Ctrl");
    click(view, "C");
    expect(lastBuilt(view)).toBe(0x0106);
  });

  it("builds right-hand modifiers", () => {
    const view = render("modified");
    click(view, "Ctrl");
    click(view, "Right hand");
    click(view, "C");
    expect(lastBuilt(view)).toBe(0x1106);
  });

  it("builds a mod-tap from ticked modifiers and a tap key", () => {
    const view = render("modTap");
    click(view, "Shift");
    click(view, "A");
    expect(lastBuilt(view)).toBe(0x2204);
  });

  it("builds a layer-tap from a layer and a tap key", () => {
    const view = render("layerTap");
    chooseLayer(view, 1);
    click(view, "SPC");
    expect(lastBuilt(view)).toBe(0x412c);
  });

  it("builds a layer-mod from a layer and ticked modifiers", () => {
    const view = render("layerMod");
    chooseLayer(view, 1);
    click(view, "Ctrl");
    expect(lastBuilt(view)).toBe(0x5021);
  });

  it("builds nothing until every part is chosen", () => {
    const view = render("modified");
    click(view, "C");
    expect(lastBuilt(view)).toBeNull();
  });

  it("builds nothing once the last modifier is unticked", () => {
    const view = render("modified");
    click(view, "Ctrl");
    click(view, "C");
    click(view, "Ctrl");
    expect(lastBuilt(view)).toBeNull();
  });

  it("asks for a modifier while none is ticked", () => {
    const view = render("modTap");
    click(view, "A");
    expect(missing(view)).toBe("Tick at least one modifier.");
  });

  it("asks for the key while none is chosen", () => {
    const view = render("modified");
    click(view, "Ctrl");
    expect(missing(view)).toBe("Choose the key.");
  });

  it("asks for a layer while none is chosen", () => {
    const view = render("layerTap");
    click(view, "SPC");
    expect(missing(view)).toBe("Choose a layer.");
  });

  it("asks for nothing once every part is chosen", () => {
    const view = render("layerMod");
    chooseLayer(view, 2);
    click(view, "Alt");
    expect(missing(view)).toBeUndefined();
  });

  it("offers only the keyboard's layers", () => {
    const view = render("layerTap", 3);
    const layers = [...layerSelect(view).options]
      .filter((option) => !option.disabled)
      .map((option) => option.textContent?.trim());
    expect(layers).toEqual(["Layer 0", "Layer 1", "Layer 2"]);
  });

  it("offers no layer past the 16 a two-part keycode holds", () => {
    const view = render("layerMod", 32);
    expect([...layerSelect(view).options].filter((option) => !option.disabled)).toHaveLength(16);
  });

  it("offers no layer keycode as the key", () => {
    const view = render("layerTap");
    expect(() => control(view, "MO(1)")).toThrow("no control labelled MO(1)");
  });

  it("asks a layer-tap for no modifier", () => {
    const view = render("layerTap");
    expect(() => control(view, "Ctrl")).toThrow("no control labelled Ctrl");
  });

  it("asks a layer-mod for no key", () => {
    const view = render("layerMod");
    expect(view.element.querySelector('input[type="search"]')).toBeNull();
  });
});
