import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { flushSync, mount, unmount } from "svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import { EDITABLE_KEYMAP, KEYMAP, withKeycode } from "../test/synthetic-keymap";
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
  canRestoreBackup: true,
};

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
    const view = render(AFTER_ESC);
    button(view, "Undo").click();
    flushSync();
    const busy = ["Apply", "Undo", "Restore backup"].map((name) => button(view, name).disabled);
    answer(EDITABLE_KEYMAP);
    expect([busy, status(view)]).toEqual([[true, true, true], "Undoing…"]);
  });
});
