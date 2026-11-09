import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { flushSync, mount, unmount } from "svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Device, EditableKeymap } from "../lib/bindings";
import { EDITABLE_KEYMAP, KEYMAP, withKeycode } from "../test/synthetic-keymap";
import Page from "./+page.svelte";

const KEYBOARD: Device = {
  name: "synthetic V5 Max ISO keymap",
  vendorId: 0x3434,
  productId: 0x0951,
  path: "fixture:/mock/cli-keymap.toml",
  isReceiver: false,
};

const RECEIVER: Device = {
  name: "Keychron Link",
  vendorId: 0x3434,
  productId: 0xd030,
  path: "fixture:/mock/receiver.toml",
  isReceiver: true,
};

const SECOND_KEYBOARD: Device = { ...KEYBOARD, path: "fixture:/mock/second.toml" };

let mounted: Record<string, unknown> | null = null;

afterEach(() => {
  if (mounted !== null) {
    unmount(mounted);
    mounted = null;
  }
  clearMocks();
  document.body.replaceChildren();
});

function render(): HTMLElement {
  const target = document.createElement("div");
  document.body.append(target);
  mounted = mount(Page, { target });
  return target;
}

/** The device's row in the list, once the list has loaded. */
function rowOf(view: HTMLElement, device: Device): Promise<HTMLLIElement> {
  return vi.waitFor(() => {
    const found = [...view.querySelectorAll("li")].find((row) =>
      row.textContent?.includes(device.path),
    );
    if (found === undefined) {
      throw new Error(`no row for ${device.path} yet`);
    }
    return found;
  });
}

async function showKeysOf(view: HTMLElement, device: Device = KEYBOARD): Promise<void> {
  const button = (await rowOf(view, device)).querySelector<HTMLButtonElement>("button.show-keys");
  if (button === null) {
    throw new Error(`no Show keys button for ${device.path}`);
  }
  button.click();
  flushSync();
}

describe("the start page", () => {
  it("draws the keyboard of the device whose Show keys button is clicked", async () => {
    const keymapPaths: unknown[] = [];
    mockIPC((command, payload) => {
      if (command === "list_devices") {
        return [KEYBOARD];
      }
      if (command === "read_keymap") {
        keymapPaths.push((payload as { path: string }).path);
        return EDITABLE_KEYMAP;
      }
      throw new Error(`unexpected command ${command}`);
    });
    const view = render();
    await showKeysOf(view);
    await vi.waitFor(() => {
      expect(view.querySelectorAll('[role="option"]')).toHaveLength(
        KEYMAP.layers[0]?.keys.filter((key) => key.isPhysical).length ?? -1,
      );
    });
    expect(keymapPaths).toEqual([KEYBOARD.path]);
  });

  it("says why the keymap cannot be read", async () => {
    const reason = "the keymap needs the keyboard on a USB cable";
    mockIPC((command) => {
      if (command === "list_devices") {
        return [KEYBOARD];
      }
      return Promise.reject(reason);
    });
    const view = render();
    await showKeysOf(view);
    await vi.waitFor(() => {
      expect(view.querySelector('[role="alert"]')?.textContent).toBe(
        `Cannot read the keymap: ${reason}`,
      );
    });
  });

  it("offers no Show keys button for a receiver, which cannot read the keymap", async () => {
    mockIPC(() => [KEYBOARD, RECEIVER]);
    const view = render();
    const receiverRow = await rowOf(view, RECEIVER);
    expect(receiverRow.querySelector("button")).toBeNull();
    expect(receiverRow.textContent).toContain("connect the keyboard by USB cable to see its keys");
  });

  it("shows the last chosen device's keys when an earlier read answers late", async () => {
    let answerFirstRead: (keymap: EditableKeymap) => void = () => {};
    const oneLayer: EditableKeymap = {
      ...EDITABLE_KEYMAP,
      keymap: { ...KEYMAP, layerCount: 1, layers: KEYMAP.layers.slice(0, 1) },
    };
    mockIPC((command, payload) => {
      if (command === "list_devices") {
        return [KEYBOARD, SECOND_KEYBOARD];
      }
      if ((payload as { path: string }).path === KEYBOARD.path) {
        return new Promise<EditableKeymap>((resolve) => {
          answerFirstRead = resolve;
        });
      }
      return EDITABLE_KEYMAP;
    });
    const view = render();
    await showKeysOf(view, KEYBOARD);
    await showKeysOf(view, SECOND_KEYBOARD);
    await vi.waitFor(() => {
      expect(view.querySelectorAll('[role="tab"]')).toHaveLength(2);
    });
    answerFirstRead(oneLayer);
    await new Promise((resolve) => setTimeout(resolve, 0));
    flushSync();
    expect(view.querySelectorAll('[role="tab"]')).toHaveLength(2);
  });

  it("remaps the selected key and draws the keycode the keyboard read back", async () => {
    const position = { layer: 0, row: 1, column: 1 };
    const afterWrite: EditableKeymap = {
      keymap: withKeycode(KEYMAP, position, 0x0029, "ESC"),
      undoSteps: 1,
      redoSteps: 0,
      canRestoreBackup: true,
    };
    const writes: unknown[] = [];
    mockIPC((command, payload) => {
      if (command === "list_devices") {
        return [KEYBOARD];
      }
      if (command === "read_keymap") {
        return EDITABLE_KEYMAP;
      }
      if (command === "set_keycode") {
        writes.push(payload);
        return afterWrite;
      }
      throw new Error(`unexpected command ${command}`);
    });
    const view = render();
    await showKeysOf(view);
    const zKey = await vi.waitFor(() => {
      const found = view.querySelector<SVGGElement>('[data-row="1"][data-column="1"]');
      if (found === null) {
        throw new Error("the keyboard is not drawn yet");
      }
      return found;
    });
    zKey.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    flushSync();
    const escapeChoice = [...view.querySelectorAll("label.choice")]
      .find((label) => label.textContent?.trim() === "ESC")
      ?.querySelector<HTMLInputElement>("input");
    escapeChoice?.click();
    flushSync();
    [...view.querySelectorAll("button")]
      .find((button) => button.textContent?.trim() === "Apply")
      ?.click();
    await vi.waitFor(() => {
      expect(
        view.querySelector('[data-row="1"][data-column="1"]')?.getAttribute("aria-label"),
      ).toBe("Z, keycode ESC (0x0029)");
    });
    expect(writes).toEqual([{ path: KEYBOARD.path, assignment: { ...position, keycode: 0x0029 } }]);
  });

  /** Clicks the key at `row`, `column` once the keyboard is drawn. */
  async function selectKey(view: HTMLElement, row: number, column: number): Promise<void> {
    const key = await vi.waitFor(() => {
      const found = view.querySelector<SVGGElement>(`[data-row="${row}"][data-column="${column}"]`);
      if (found === null) {
        throw new Error("the keyboard is not drawn yet");
      }
      return found;
    });
    key.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    flushSync();
  }

  function buttonNamed(view: HTMLElement, name: string): HTMLButtonElement | undefined {
    return [...view.querySelectorAll("button")].find((button) =>
      button.textContent?.trim().startsWith(name),
    );
  }

  it("forgets the selected key when another device's keys are shown", async () => {
    mockIPC((command) =>
      command === "list_devices" ? [KEYBOARD, SECOND_KEYBOARD] : EDITABLE_KEYMAP,
    );
    const view = render();
    await showKeysOf(view, KEYBOARD);
    await selectKey(view, 1, 1);
    await showKeysOf(view, SECOND_KEYBOARD);
    await vi.waitFor(() => {
      expect(view.querySelector(".selected-key")?.textContent?.trim()).toBe("No key selected.");
    });
  });

  it("offers the undo the app kept for a device when its keys are shown again", async () => {
    const histories: Record<string, EditableKeymap> = {
      [KEYBOARD.path]: { ...EDITABLE_KEYMAP, undoSteps: 1, canRestoreBackup: true },
      [SECOND_KEYBOARD.path]: EDITABLE_KEYMAP,
    };
    mockIPC((command, payload) =>
      command === "list_devices"
        ? [KEYBOARD, SECOND_KEYBOARD]
        : histories[(payload as { path: string }).path],
    );
    const view = render();
    await showKeysOf(view, SECOND_KEYBOARD);
    await showKeysOf(view, KEYBOARD);
    await vi.waitFor(() => {
      expect(buttonNamed(view, "Undo")?.disabled).toBe(false);
    });
  });
});
