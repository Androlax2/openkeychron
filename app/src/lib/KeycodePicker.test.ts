import { flushSync, mount, unmount } from "svelte";
import { afterEach, describe, expect, it } from "vitest";
import KeycodePicker from "./KeycodePicker.svelte";

let mounted: Record<string, unknown> | null = null;

afterEach(() => {
  if (mounted !== null) {
    unmount(mounted);
    mounted = null;
  }
  document.body.replaceChildren();
});

function render(
  chosen: number | null = null,
  onchoose: (keycode: number) => void = () => {},
): HTMLElement {
  const target = document.createElement("div");
  document.body.append(target);
  mounted = mount(KeycodePicker, { target, props: { layerCount: 4, chosen, onchoose } });
  flushSync();
  return target;
}

function search(view: HTMLElement, text: string): void {
  const input = view.querySelector<HTMLInputElement>('input[type="search"]');
  if (input === null) {
    throw new Error("no search field");
  }
  input.value = text;
  input.dispatchEvent(new Event("input", { bubbles: true }));
  flushSync();
}

function groups(view: HTMLElement): string[] {
  return [...view.querySelectorAll("fieldset legend")].map(
    (legend) => legend.textContent?.trim() ?? "",
  );
}

function choiceNames(view: HTMLElement): string[] {
  return [...view.querySelectorAll("label.choice")].map((label) => label.textContent?.trim() ?? "");
}

function radioNamed(view: HTMLElement, name: string): HTMLInputElement {
  const label = [...view.querySelectorAll("label.choice")].find(
    (candidate) => candidate.textContent?.trim() === name,
  );
  const radio = label?.querySelector<HTMLInputElement>('input[type="radio"]');
  if (radio === null || radio === undefined) {
    throw new Error(`no keycode choice named ${name}`);
  }
  return radio;
}

describe("KeycodePicker", () => {
  it("groups the keycodes by category", () => {
    const view = render();
    expect(groups(view).slice(0, 4)).toEqual(["Special", "Letters", "Numbers", "Editing"]);
  });

  it("labels the search field", () => {
    const view = render();
    const input = view.querySelector<HTMLInputElement>('input[type="search"]');
    expect(view.querySelector(`label[for="${input?.id}"]`)?.textContent?.trim()).toBe(
      "Search keycodes",
    );
  });

  it("shows only the keycodes that match the search", () => {
    const view = render();
    search(view, "bspc");
    expect([groups(view), choiceNames(view)]).toEqual([["Editing"], ["BSPC"]]);
  });

  it("offers layer keycodes only for the keyboard's layers", () => {
    const view = render();
    search(view, "MO(");
    expect(choiceNames(view)).toEqual(["MO(0)", "MO(1)", "MO(2)", "MO(3)"]);
  });

  it("says so when no keycode matches", () => {
    const view = render();
    search(view, "nothing like this");
    expect(view.querySelector(".no-match")?.textContent?.trim()).toBe(
      "No keycode matches “nothing like this”.",
    );
  });

  it("reports the keycode the user chooses", () => {
    const chosen: number[] = [];
    const view = render(null, (keycode) => chosen.push(keycode));
    radioNamed(view, "ESC").click();
    flushSync();
    expect(chosen).toEqual([0x0029]);
  });

  it("checks the chosen keycode", () => {
    const view = render(0x0029);
    expect(
      [...view.querySelectorAll<HTMLInputElement>('input[type="radio"]:checked')].map(
        (radio) => radio.value,
      ),
    ).toEqual(["41"]);
  });
});
