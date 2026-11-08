// Generated from app/src-tauri by `OPENKEYCHRON_WRITE_BINDINGS=1 cargo test -p openkeychron-app bindings`. Do not edit.

export type Device = { 
/**
 * The USB product string, when the device reports one.
 */
name: string | null, vendorId: number, productId: number, 
/**
 * What the other commands take to name this device.
 */
path: string, 
/**
 * True for a USB receiver such as the Keychron Link: the keyboard behind it is wireless.
 */
isReceiver: boolean, };

export type DeviceInfo = { viaProtocolVersion: number, viaFirmwareVersion: number, layoutOptions: number, uptimeMs: number, keychronFirmware: KeychronFirmware, };

export type KeychronFirmware = { "kind": "reported", version: string, buildDate: string, } | { "kind": "notReported", reason: string, };

export type Keymap = { layerCount: number, rows: number, columns: number, layers: Array<Layer>, };

export type Layer = { index: number, 
/**
 * Row by row, `columns` keys per row.
 */
keys: Array<Key>, };

export type Key = { row: number, column: number, 
/**
 * The physical French legend, which names the position, not what the key now types.
 */
legend: string | null, 
/**
 * False for matrix slots no physical key uses.
 */
isPhysical: boolean, keycode: number, };

export type MacroList = { macroCount: number, bufferSize: number, 
/**
 * False when the buffer's last byte is not zero: the firmware then plays no macro.
 */
macrosEnabled: boolean, macros: Array<MacroSlot>, };

export type MacroSlot = { slot: number, bytes: Array<number>, content: MacroContent, };

export type MacroContent = { "kind": "actions", actions: Array<Action>, } | { "kind": "unreadable", error: string, };

export type Action = { "kind": "text", "value": string } | { "kind": "tap", "value": number } | { "kind": "down", "value": number } | { "kind": "up", "value": number } | { "kind": "delay", "value": number };

export type Settings = { debounce: Debounce, 
/**
 * Every pair the firmware holds, `off` ones included, in firmware order from pair 0.
 */
snapClickPairs: Array<SnapClickPair>, wireless: WirelessTimers, };

export type Debounce = { algorithm: DebounceAlgorithm, timeMs: number, };

export type DebounceAlgorithm = "symmetric-defer-global" | "symmetric-defer-per-row" | "symmetric-defer-per-key" | "symmetric-eager-per-row" | "symmetric-eager-per-key" | "asymmetric-eager-defer-per-key" | "none";

export type SnapClickPair = { mode: SnapClickMode, 
/**
 * Two 8-bit basic keycodes.
 */
keys: [number, number], };

export type SnapClickMode = "off" | "regular" | "last-input" | "first-key" | "second-key" | "neutral";

export type WirelessTimers = { backlightOffSeconds: number, sleepSeconds: number, };
