// The Tauri commands in app/src-tauri/src/commands.rs. Each rejects with the error's message,
// a string ready to show.
import { invoke } from "@tauri-apps/api/core";
import type { Device, DeviceInfo, Keymap, MacroList, Settings } from "./bindings";

export const listDevices = (): Promise<Device[]> => invoke("list_devices");

/** `path` is a `Device.path` from `listDevices`. */
export const readInfo = (path: string): Promise<DeviceInfo> => invoke("read_info", { path });

/** Refused for a receiver: it does not identify the keyboard behind it. */
export const readKeymap = (path: string): Promise<Keymap> => invoke("read_keymap", { path });

export const readMacros = (path: string): Promise<MacroList> => invoke("read_macros", { path });

export const readSettings = (path: string): Promise<Settings> => invoke("read_settings", { path });
