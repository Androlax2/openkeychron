// Runes, so a test can change the panel's selection after it is mounted, as the page does when
// another key is clicked.
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { flushSync, mount, unmount } from "svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import { EDITABLE_KNOB, KNOB } from "../test/synthetic-keymap";
import type { EditableKeymap } from "./bindings";
import type { KeySelection } from "./Keyboard.svelte";
import RemapPanel from "./RemapPanel.svelte";

const PATH = "fixture:/mock/keyboard.mock.toml";
/** The key labelled Z, which no encoder uses. */
const Z_KEY: KeySelection = { layer: 0, row: 1, column: 1 };
const KC_ESC = 0x0029;

let mounted: Record<string, unknown> | null = null;

afterEach(() => {
  if (mounted !== null) {
    unmount(mounted);
    mounted = null;
  }
  clearMocks();
  document.body.replaceChildren();
});

function render(props: {
  path: string;
  editable: EditableKeymap;
  selection: KeySelection | null;
  onchange: (editable: EditableKeymap) => void;
}): HTMLElement {
  const target = document.createElement("div");
  document.body.append(target);
  mounted = mount(RemapPanel, { target, props });
  flushSync();
  return target;
}

function radio(view: HTMLElement, fieldset: string, name: string): HTMLInputElement {
  const label = [...view.querySelectorAll(`fieldset.${fieldset} label`)].find((candidate) =>
    candidate.textContent?.trim().startsWith(name),
  );
  const input = label?.querySelector<HTMLInputElement>('input[type="radio"]');
  if (input === null || input === undefined) {
    throw new Error(`no ${name} in ${fieldset}`);
  }
  return input;
}

function chooseEscape(view: HTMLElement): void {
  const label = [...view.querySelectorAll("label.choice")].find(
    (candidate) => candidate.textContent?.trim() === "ESC",
  );
  label?.querySelector<HTMLInputElement>("input")?.click();
  flushSync();
}

describe("RemapPanel, as the selection changes", () => {
  function knobWithClockwiseChosen() {
    const props = $state({
      path: PATH,
      editable: EDITABLE_KNOB,
      selection: KNOB as KeySelection | null,
      onchange: () => {},
    });
    const view = render(props);
    radio(view, "target", "Turn clockwise").click();
    flushSync();
    return { props, view };
  }

  it("goes back to the press once another key is selected and the knob again", () => {
    const { props, view } = knobWithClockwiseChosen();
    props.selection = Z_KEY;
    flushSync();
    props.selection = KNOB;
    flushSync();
    expect(radio(view, "target", "Press").checked).toBe(true);
  });

  it("keeps the chosen turn when the knob is shown on another layer", () => {
    const { props, view } = knobWithClockwiseChosen();
    props.selection = { ...KNOB, layer: 1 };
    flushSync();
    expect(radio(view, "target", "Turn clockwise").checked).toBe(true);
  });

  it("sets the newly selected key's press after a turn was chosen on the knob", async () => {
    const calls: unknown[] = [];
    mockIPC((command, payload) => {
      calls.push([command, payload]);
      return EDITABLE_KNOB;
    });
    const { props, view } = knobWithClockwiseChosen();
    props.selection = Z_KEY;
    flushSync();
    chooseEscape(view);
    [...view.querySelectorAll("button")]
      .find((candidate) => candidate.textContent?.trim() === "Apply")
      ?.click();
    await vi.waitFor(() => expect(calls).toHaveLength(1));
    expect(calls).toEqual([
      ["set_keycode", { path: PATH, assignment: { ...Z_KEY, keycode: KC_ESC } }],
    ]);
  });
});

describe("RemapPanel history help", () => {
  it("says the history ends when the keyboard is unplugged, not only when the app quits", () => {
    const view = render({
      path: PATH,
      editable: EDITABLE_KNOB,
      selection: Z_KEY,
      onchange: () => {},
    });
    const help = view.querySelector("[id$='-history']")?.textContent?.replace(/\s+/g, " ") ?? "";
    expect(help).toMatch(/until the app quits or the keyboard is unplugged/);
  });
});
