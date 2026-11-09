import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { flushSync, mount, unmount } from "svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  EDITABLE_KEYMAP,
  EDITABLE_KNOB,
  KEYMAP,
  KNOB,
  KNOB_KEYMAP,
  withKeycode,
} from "../test/synthetic-keymap";
import type { EditableKeymap } from "./bindings";
import type { KeySelection } from "./Keyboard.svelte";
import RemapPanel from "./RemapPanel.svelte";

const PATH = "fixture:/mock/keyboard.mock.toml";
/** The key labelled Z, which types W on layer 0. */
const Z_KEY: KeySelection = { layer: 0, row: 1, column: 1 };
const KC_ESC = 0x0029;

const AFTER_ESC: EditableKeymap = {
  keymap: withKeycode(KEYMAP, Z_KEY, KC_ESC, "ESC"),
  undoSteps: 1,
  redoSteps: 0,
  canRestoreBackup: true,
};

const KNOB_TURNED_TO_ESC: EditableKeymap = {
  ...EDITABLE_KNOB,
  keymap: {
    ...KNOB_KEYMAP,
    layers: KNOB_KEYMAP.layers.map((layer) =>
      layer.index !== 0
        ? layer
        : {
            ...layer,
            encoders: layer.encoders.map((encoder) => ({
              ...encoder,
              clockwise: { keycode: KC_ESC, name: "ESC" },
            })),
          },
    ),
  },
  undoSteps: 1,
  canRestoreBackup: true,
};

/** AFTER_ESC once Undo took the change back. */
const UNDONE_ESC: EditableKeymap = { ...EDITABLE_KEYMAP, redoSteps: 1, canRestoreBackup: true };

let mounted: Record<string, unknown> | null = null;

afterEach(() => {
  if (mounted !== null) {
    unmount(mounted);
    mounted = null;
  }
  clearMocks();
  document.body.replaceChildren();
});

function render(
  editable: EditableKeymap = EDITABLE_KEYMAP,
  selection: KeySelection | null = Z_KEY,
  onchange: (editable: EditableKeymap) => void = () => {},
): HTMLElement {
  const target = document.createElement("div");
  document.body.append(target);
  mounted = mount(RemapPanel, { target, props: { path: PATH, editable, selection, onchange } });
  flushSync();
  return target;
}

function button(view: HTMLElement, name: string): HTMLButtonElement {
  const found = [...view.querySelectorAll("button")].find((candidate) =>
    candidate.textContent?.trim().startsWith(name),
  );
  if (found === undefined) {
    throw new Error(`no ${name} button`);
  }
  return found;
}

function choose(view: HTMLElement, name: string): void {
  const label = [...view.querySelectorAll("label.choice")].find(
    (candidate) => candidate.textContent?.trim() === name,
  );
  const radio = label?.querySelector<HTMLInputElement>('input[type="radio"]');
  if (radio === null || radio === undefined) {
    throw new Error(`no keycode choice named ${name}`);
  }
  radio.click();
  flushSync();
}

function status(view: HTMLElement): string | undefined {
  return view.querySelector('[role="status"]')?.textContent?.trim();
}

function alertText(view: HTMLElement): string | undefined {
  return view.querySelector('[role="alert"]')?.textContent?.trim();
}

function chooseKind(view: HTMLElement, kind: string): void {
  const label = [...view.querySelectorAll("fieldset.kind label")].find(
    (candidate) => candidate.textContent?.trim() === kind,
  );
  const radio = label?.querySelector<HTMLInputElement>('input[type="radio"]');
  if (radio === null || radio === undefined) {
    throw new Error(`no keycode kind ${kind}`);
  }
  radio.click();
  flushSync();
}

function typeRaw(view: HTMLElement, text: string): void {
  const input = view.querySelector<HTMLInputElement>(".raw input");
  if (input === null) {
    throw new Error("no raw keycode field");
  }
  input.value = text;
  input.dispatchEvent(new Event("input", { bubbles: true }));
  flushSync();
}

describe("RemapPanel", () => {
  it("names the selected key and the keycode it has now", () => {
    const view = render();
    expect(view.querySelector(".selected-key")?.textContent?.trim()).toBe(
      "Layer 0, Z (row 1, column 1): W (0x001a)",
    );
  });

  function applyHint(view: HTMLElement): string | undefined {
    return view.querySelector(".apply-hint")?.textContent?.trim();
  }

  it("cannot apply before a key is selected", () => {
    const view = render(EDITABLE_KEYMAP, null);
    choose(view, "ESC");
    expect(button(view, "Apply").disabled).toBe(true);
  });

  it("asks for a key when none is selected", () => {
    const view = render(EDITABLE_KEYMAP, null);
    choose(view, "ESC");
    expect(applyHint(view)).toBe("Select a key on the keyboard first.");
  });

  it("cannot apply before a keycode is chosen", () => {
    const view = render();
    expect(button(view, "Apply").disabled).toBe(true);
  });

  it("asks for a keycode when none is chosen", () => {
    const view = render();
    expect(applyHint(view)).toBe("Choose a keycode for it.");
  });

  it("cannot apply the keycode the key already has", () => {
    const view = render();
    choose(view, "W");
    expect(button(view, "Apply").disabled).toBe(true);
  });

  it("says the key already has the chosen keycode", () => {
    const view = render();
    choose(view, "W");
    expect(applyHint(view)).toBe("The key already has this keycode.");
  });

  it("sets the selected key to the chosen keycode", async () => {
    const calls: unknown[] = [];
    mockIPC((command, payload) => {
      calls.push([command, payload]);
      return AFTER_ESC;
    });
    const view = render();
    choose(view, "ESC");
    button(view, "Apply").click();
    await vi.waitFor(() => expect(calls).toHaveLength(1));
    expect(calls).toEqual([
      ["set_keycode", { path: PATH, assignment: { ...Z_KEY, keycode: KC_ESC } }],
    ]);
  });

  it("shows the key as the keyboard read it back after applying", async () => {
    mockIPC(() => AFTER_ESC);
    const view = render();
    choose(view, "ESC");
    button(view, "Apply").click();
    await vi.waitFor(() => {
      expect(status(view)).toBe(
        "Applied, and the whole keymap read back as expected. Read back: layer 0, Z (row 1, " +
          "column 1) is ESC (0x0029).",
      );
    });
  });

  it("hands the keymap read back after a write to its owner", async () => {
    const changes: EditableKeymap[] = [];
    mockIPC(() => AFTER_ESC);
    const view = render(EDITABLE_KEYMAP, Z_KEY, (editable) => changes.push(editable));
    choose(view, "ESC");
    button(view, "Apply").click();
    await vi.waitFor(() => expect(changes).toEqual([AFTER_ESC]));
  });

  it("says why a write failed", async () => {
    const reason =
      "device I/O failed: unplugged; the keyboard may have changed: Undo puts back the keymap " +
      "from before this change";
    mockIPC((command) => (command === "read_keymap" ? EDITABLE_KEYMAP : Promise.reject(reason)));
    const view = render();
    choose(view, "ESC");
    button(view, "Apply").click();
    await vi.waitFor(() => expect(alertText(view)).toBe(`Cannot apply: ${reason}`));
  });

  it("after a failed write, offers the undo the app kept, read again from the app", async () => {
    const changes: EditableKeymap[] = [];
    const kept: EditableKeymap = { ...EDITABLE_KEYMAP, undoSteps: 1, canRestoreBackup: true };
    mockIPC((command) => (command === "read_keymap" ? kept : Promise.reject("unplugged")));
    const view = render(EDITABLE_KEYMAP, Z_KEY, (editable) => changes.push(editable));
    choose(view, "ESC");
    button(view, "Apply").click();
    await vi.waitFor(() => expect(changes).toEqual([kept]));
  });

  it("says so when the keymap cannot be read again after a failed write", async () => {
    mockIPC(() => Promise.reject("unplugged"));
    const view = render();
    choose(view, "ESC");
    button(view, "Apply").click();
    await vi.waitFor(() =>
      expect(alertText(view)).toBe(
        "Cannot apply: unplugged. Reading the keymap again also failed: unplugged",
      ),
    );
  });

  it("offers no undo and no restore before the app changed anything", () => {
    const view = render();
    expect([button(view, "Undo").disabled, button(view, "Restore backup").disabled]).toEqual([
      true,
      true,
    ]);
  });

  it("says how many changes undo can take back", () => {
    const view = render({ ...AFTER_ESC, undoSteps: 2 });
    expect(button(view, "Undo").textContent?.trim()).toBe("Undo (2)");
  });

  it("undoes the newest change and shows the key read back", async () => {
    const calls: unknown[] = [];
    mockIPC((command, payload) => {
      calls.push([command, payload]);
      return { ...EDITABLE_KEYMAP, canRestoreBackup: true };
    });
    const view = render(AFTER_ESC);
    button(view, "Undo").click();
    await vi.waitFor(() => {
      expect(status(view)).toBe(
        "Undone, and the whole keymap read back as expected. Read back: layer 0, Z (row 1, " +
          "column 1) is W (0x001a).",
      );
    });
    expect(calls).toEqual([["undo_keymap_change", { path: PATH }]]);
  });

  it("offers no redo before an undo", () => {
    const view = render(AFTER_ESC);
    expect(button(view, "Redo").disabled).toBe(true);
  });

  it("says how many changes redo can put back", () => {
    const view = render({ ...UNDONE_ESC, redoSteps: 2 });
    expect(button(view, "Redo").textContent?.trim()).toBe("Redo (2)");
  });

  it("redoes the change the newest undo took back and shows the key read back", async () => {
    const calls: unknown[] = [];
    mockIPC((command, payload) => {
      calls.push([command, payload]);
      return AFTER_ESC;
    });
    const view = render(UNDONE_ESC);
    button(view, "Redo").click();
    await vi.waitFor(() => {
      expect(status(view)).toBe(
        "Redone, and the whole keymap read back as expected. Read back: layer 0, Z (row 1, " +
          "column 1) is ESC (0x0029).",
      );
    });
    expect(calls).toEqual([["redo_keymap_change", { path: PATH }]]);
  });

  it("restores the backup from before the first change", async () => {
    const calls: unknown[] = [];
    mockIPC((command, payload) => {
      calls.push([command, payload]);
      return { ...EDITABLE_KEYMAP, undoSteps: 2, canRestoreBackup: true };
    });
    const view = render(AFTER_ESC);
    button(view, "Restore backup").click();
    await vi.waitFor(() => {
      expect(status(view)).toBe(
        "Backup restored, and the whole keymap read back as expected. Read back: layer 0, Z " +
          "(row 1, column 1) is W (0x001a).",
      );
    });
    expect(calls).toEqual([["restore_keymap_backup", { path: PATH }]]);
  });

  it("allows one write at a time", async () => {
    let answer: (editable: EditableKeymap) => void = () => {};
    mockIPC(
      () =>
        new Promise<EditableKeymap>((resolve) => {
          answer = resolve;
        }),
    );
    const view = render({ ...AFTER_ESC, redoSteps: 1 });
    button(view, "Undo").click();
    flushSync();
    const busy = ["Apply", "Undo", "Redo", "Restore backup"].map(
      (name) => button(view, name).disabled,
    );
    answer(EDITABLE_KEYMAP);
    expect([busy, status(view)]).toEqual([[true, true, true, true], "Undoing…"]);
  });

  describe("building a keycode", () => {
    function tick(view: HTMLElement, modifier: string): void {
      const label = [...view.querySelectorAll("label")].find(
        (candidate) => candidate.textContent?.trim() === modifier,
      );
      label?.querySelector("input")?.click();
      flushSync();
    }

    function chosenText(view: HTMLElement): string | undefined {
      return view.querySelector(".chosen")?.textContent?.trim();
    }

    it("offers each kind of keycode", () => {
      const view = render();
      expect(
        [...view.querySelectorAll("fieldset.kind label")].map((label) => label.textContent?.trim()),
      ).toEqual([
        "Keycode",
        "Modifiers + key",
        "Mod-tap",
        "Layer-tap",
        "Layer + modifiers",
        "Raw value",
      ]);
    });

    it("names a built modifier combination before applying it", () => {
      const view = render();
      chooseKind(view, "Modifiers + key");
      tick(view, "Ctrl");
      choose(view, "C");
      expect(chosenText(view)).toBe("New keycode: LCTL(C) (0x0106)");
    });

    it("applies a built modifier combination", async () => {
      const calls: unknown[] = [];
      mockIPC((command, payload) => {
        calls.push([command, payload]);
        return AFTER_ESC;
      });
      const view = render();
      chooseKind(view, "Modifiers + key");
      tick(view, "Ctrl");
      choose(view, "C");
      button(view, "Apply").click();
      await vi.waitFor(() => expect(calls).toHaveLength(1));
      expect(calls).toEqual([
        ["set_keycode", { path: PATH, assignment: { ...Z_KEY, keycode: 0x0106 } }],
      ]);
    });

    it("names a built mod-tap by QMK's shorthand", () => {
      const view = render();
      chooseKind(view, "Mod-tap");
      tick(view, "Shift");
      choose(view, "A");
      expect(chosenText(view)).toBe("New keycode: LSFT_T(A) (0x2204)");
    });

    it("cannot apply a combination still missing a part", () => {
      const view = render();
      chooseKind(view, "Modifiers + key");
      choose(view, "C");
      expect(button(view, "Apply").disabled).toBe(true);
    });

    it("forgets the chosen keycode when the kind changes", () => {
      const view = render();
      choose(view, "ESC");
      chooseKind(view, "Raw value");
      expect(chosenText(view)).toBe("New keycode: none chosen");
    });

    it("forgets the built keycode when switching to another builder", () => {
      const view = render();
      chooseKind(view, "Modifiers + key");
      tick(view, "Ctrl");
      choose(view, "C");
      chooseKind(view, "Mod-tap");
      expect(chosenText(view)).toBe("New keycode: none chosen");
    });

    function tickConfirmation(view: HTMLElement): void {
      const confirmation = view.querySelector<HTMLInputElement>(".raw .risky input");
      if (confirmation === null) {
        throw new Error("no confirmation for a risky keycode");
      }
      confirmation.click();
      flushSync();
    }

    it.each([["7c00"], ["7c01"], ["7c03"]])(
      "warns before mapping a key to a keycode that resets the keyboard (%s)",
      (text) => {
        const view = render();
        chooseKind(view, "Raw value");
        typeRaw(view, text);
        expect(view.querySelector(".raw .risky")?.textContent).toContain(
          "Pressing this key restarts the keyboard, enters its bootloader or erases its settings",
        );
      },
    );

    it("cannot apply a keycode that resets the keyboard until it is confirmed", () => {
      const view = render();
      chooseKind(view, "Raw value");
      typeRaw(view, "0x7c00");
      expect([button(view, "Apply").disabled, applyHint(view)]).toEqual([
        true,
        "Confirm that this key may reset the keyboard.",
      ]);
    });

    it("applies a keycode that resets the keyboard once it is confirmed", async () => {
      const calls: unknown[] = [];
      mockIPC((command, payload) => {
        calls.push([command, payload]);
        return AFTER_ESC;
      });
      const view = render();
      chooseKind(view, "Raw value");
      typeRaw(view, "0x7c00");
      tickConfirmation(view);
      button(view, "Apply").click();
      await vi.waitFor(() => expect(calls).toHaveLength(1));
      expect(calls).toEqual([
        ["set_keycode", { path: PATH, assignment: { ...Z_KEY, keycode: 0x7c00 } }],
      ]);
    });

    it("asks again for the confirmation when the raw keycode changes", () => {
      const view = render();
      chooseKind(view, "Raw value");
      typeRaw(view, "0x7c00");
      tickConfirmation(view);
      typeRaw(view, "0x7c03");
      expect(button(view, "Apply").disabled).toBe(true);
    });

    it("names a raw keycode before applying it", () => {
      const view = render();
      chooseKind(view, "Raw value");
      typeRaw(view, "0x412c");
      expect(chosenText(view)).toBe("New keycode: LT(1, SPC) (0x412c)");
    });

    it("shows a raw keycode QMK does not name by its value", () => {
      const view = render();
      chooseKind(view, "Raw value");
      typeRaw(view, "7e05");
      expect(chosenText(view)).toBe("New keycode: 0x7e05");
    });

    it("applies a raw keycode", async () => {
      const calls: unknown[] = [];
      mockIPC((command, payload) => {
        calls.push([command, payload]);
        return AFTER_ESC;
      });
      const view = render();
      chooseKind(view, "Raw value");
      typeRaw(view, "7e05");
      button(view, "Apply").click();
      await vi.waitFor(() => expect(calls).toHaveLength(1));
      expect(calls).toEqual([
        ["set_keycode", { path: PATH, assignment: { ...Z_KEY, keycode: 0x7e05 } }],
      ]);
    });

    it("says how to write a raw keycode it cannot read", () => {
      const view = render();
      chooseKind(view, "Raw value");
      typeRaw(view, "0x12345");
      expect(view.querySelector(".raw .invalid")?.textContent?.trim()).toBe(
        "Enter one to four hexadecimal digits, such as 0x0106.",
      );
    });

    it("cannot apply a raw keycode it cannot read", () => {
      const view = render();
      chooseKind(view, "Raw value");
      typeRaw(view, "0x12345");
      expect(button(view, "Apply").disabled).toBe(true);
    });
  });

  describe("on the knob", () => {
    function targets(view: HTMLElement): string[] {
      return [...view.querySelectorAll(".target label")].map(
        (label) => label.textContent?.trim().replace(/\s+/g, " ") ?? "",
      );
    }

    function chooseTarget(view: HTMLElement, name: string): void {
      const label = [...view.querySelectorAll(".target label")].find((candidate) =>
        candidate.textContent?.trim().startsWith(name),
      );
      const radio = label?.querySelector<HTMLInputElement>('input[type="radio"]');
      if (radio === null || radio === undefined) {
        throw new Error(`no target named ${name}`);
      }
      radio.click();
      flushSync();
    }

    it("offers its press and both turns, with what each sends now", () => {
      const view = render(EDITABLE_KNOB, KNOB);
      expect(targets(view)).toEqual([
        "Press: NUM (0x0053)",
        "Turn counter-clockwise: VOLD (0x00aa)",
        "Turn clockwise: VOLU (0x00a9)",
      ]);
    });

    it("shows what the turns send on the selected layer", () => {
      const view = render(EDITABLE_KNOB, { ...KNOB, layer: 1 });
      expect(targets(view).slice(1)).toEqual([
        "Turn counter-clockwise: TRNS (0x0001)",
        "Turn clockwise: TRNS (0x0001)",
      ]);
    });

    it("offers no turns for a key that is no encoder's press", () => {
      const view = render(EDITABLE_KNOB, Z_KEY);
      expect(targets(view)).toEqual([]);
    });

    it("names the chosen turn and what it sends now", () => {
      const view = render(EDITABLE_KNOB, KNOB);
      chooseTarget(view, "Turn clockwise");
      expect(view.querySelector(".selected-key")?.textContent?.trim()).toBe(
        "Layer 0, Knob (row 0, column 4) turned clockwise: VOLU (0x00a9)",
      );
    });

    it("says the turn already sends the chosen keycode", () => {
      const view = render(EDITABLE_KNOB, KNOB);
      chooseTarget(view, "Turn clockwise");
      choose(view, "VOLU");
      expect(view.querySelector(".apply-hint")?.textContent?.trim()).toBe(
        "The turn already sends this keycode.",
      );
    });

    it("sets what turning the knob sends to the chosen keycode", async () => {
      const calls: unknown[] = [];
      mockIPC((command, payload) => {
        calls.push([command, payload]);
        return KNOB_TURNED_TO_ESC;
      });
      const view = render(EDITABLE_KNOB, KNOB);
      chooseTarget(view, "Turn clockwise");
      choose(view, "ESC");
      button(view, "Apply").click();
      await vi.waitFor(() => expect(calls).toHaveLength(1));
      expect(calls).toEqual([
        [
          "set_encoder_keycode",
          {
            path: PATH,
            assignment: { layer: 0, encoder: 0, direction: "clockwise", keycode: KC_ESC },
          },
        ],
      ]);
    });

    it("shows the turn as the keyboard read it back after applying", async () => {
      mockIPC(() => KNOB_TURNED_TO_ESC);
      const view = render(EDITABLE_KNOB, KNOB);
      chooseTarget(view, "Turn clockwise");
      choose(view, "ESC");
      button(view, "Apply").click();
      await vi.waitFor(() => {
        expect(status(view)).toBe(
          "Applied, and the whole keymap read back as expected. Read back: layer 0, Knob (row " +
            "0, column 4) turned clockwise is ESC (0x0029).",
        );
      });
    });

    it("warns before a turn sends a keycode that resets the keyboard", () => {
      const view = render(EDITABLE_KNOB, KNOB);
      chooseTarget(view, "Turn clockwise");
      chooseKind(view, "Raw value");
      typeRaw(view, "7c00");
      expect(view.querySelector(".raw .risky")?.textContent).toContain(
        "Turning the knob this way restarts the keyboard, enters its bootloader or erases its " +
          "settings",
      );
    });

    it("asks to confirm that a turn may reset the keyboard", () => {
      const view = render(EDITABLE_KNOB, KNOB);
      chooseTarget(view, "Turn clockwise");
      chooseKind(view, "Raw value");
      typeRaw(view, "7c00");
      expect(view.querySelector(".apply-hint")?.textContent?.trim()).toBe(
        "Confirm that this turn may reset the keyboard.",
      );
    });

    it("still sets the knob's press while Press is chosen", async () => {
      const calls: unknown[] = [];
      mockIPC((command, payload) => {
        calls.push([command, payload]);
        return EDITABLE_KNOB;
      });
      const view = render(EDITABLE_KNOB, KNOB);
      choose(view, "ESC");
      button(view, "Apply").click();
      await vi.waitFor(() => expect(calls).toHaveLength(1));
      expect(calls).toEqual([
        ["set_keycode", { path: PATH, assignment: { ...KNOB, keycode: KC_ESC } }],
      ]);
    });
  });
});
