// The Tauri commands in app/src-tauri/src/commands.rs. Each rejects with the error's message,
// a string ready to show.
import { invoke } from "@tauri-apps/api/core";
import type {
  Device,
  DeviceInfo,
  EditableKeymap,
  KeyAssignment,
  MacroList,
  Settings,
} from "./bindings";

export const listDevices = (): Promise<Device[]> => invoke("list_devices");

/** `path` is a `Device.path` from `listDevices`. */
export const readInfo = (path: string): Promise<DeviceInfo> => invoke("read_info", { path });

/** Refused for a receiver: it does not identify the keyboard behind it. */
export const readKeymap = (path: string): Promise<EditableKeymap> =>
  invoke("read_keymap", { path });

// The four keymap writes. Each goes through the guarded write path: refused for a receiver or
// an unverified model, the keymap it replaces kept in memory first, the whole keymap read back.
// Each resolves with the keymap read again after the write.

export const setKeycode = (path: string, assignment: KeyAssignment): Promise<EditableKeymap> =>
  invoke("set_keycode", { path, assignment });

/** Takes back the newest keymap change the app made on the device. */
export const undoKeymapChange = (path: string): Promise<EditableKeymap> =>
  invoke("undo_keymap_change", { path });

/** Puts back the keymap change the newest undo on the device took back. */
export const redoKeymapChange = (path: string): Promise<EditableKeymap> =>
  invoke("redo_keymap_change", { path });

/** Puts back the keymap from before the app's first change on the device. */
export const restoreKeymapBackup = (path: string): Promise<EditableKeymap> =>
  invoke("restore_keymap_backup", { path });

export const readMacros = (path: string): Promise<MacroList> => invoke("read_macros", { path });

export const readSettings = (path: string): Promise<Settings> => invoke("read_settings", { path });
