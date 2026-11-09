import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { flushSync, mount, unmount } from "svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Device, Keymap } from "../lib/bindings";
import { KEYMAP } from "../test/synthetic-keymap";
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
        return KEYMAP;
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
    let answerFirstRead: (keymap: Keymap) => void = () => {};
    const oneLayer: Keymap = { ...KEYMAP, layerCount: 1, layers: KEYMAP.layers.slice(0, 1) };
    mockIPC((command, payload) => {
      if (command === "list_devices") {
        return [KEYBOARD, SECOND_KEYBOARD];
      }
      if ((payload as { path: string }).path === KEYBOARD.path) {
        return new Promise<Keymap>((resolve) => {
          answerFirstRead = resolve;
        });
      }
      return KEYMAP;
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
});
