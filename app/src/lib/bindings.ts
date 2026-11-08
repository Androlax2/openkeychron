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
 * The keycap's Shift legend, such as `2` on the `é` key.
 */
shiftLegend: string | null, 
/**
 * The keycap's AltGr legend, such as `~` on the `é` key.
 */
altGrLegend: string | null, 
/**
 * False for matrix slots no physical key uses.
 */
isPhysical: boolean, 
/**
 * Where the key sits, in key units from the keyboard's top-left corner. Absent for a
 * matrix slot no physical key uses.
 */
bounds: Bounds | null, 
/**
 * A second rectangle that, with `bounds`, makes a shape such as ISO Enter.
 */
secondaryBounds: Bounds | null, keycode: number, 
/**
 * QMK's name for `keycode`, such as `LCTL(C)`, or the connected model's name for a
 * Keychron custom keycode, such as `LOPTN`. Absent for any other custom or unknown
 * keycode: show `keycode` then.
 */
name: string | null, };

export type Bounds = { x: number, y: number, width: number, height: number, };

export type EditableKeymap = { keymap: Keymap, 
/**
 * How many changes Undo can take back, one at a time, newest first.
 */
undoSteps: number, 
/**
 * True once the app holds the keymap from before its first change on this device.
 */
canRestoreBackup: boolean, };

export type KeyAssignment = { layer: number, row: number, column: number, keycode: number, };

export type KeycodeCategory = { name: string, keycodes: Array<KeycodeChoice>, };

export type KeycodeChoice = { keycode: number, 
/**
 * QMK's name, such as `ESC` or `MO(1)`.
 */
name: string, 
/**
 * The layer a layer keycode switches to, so the UI can offer only the keyboard's layers.
 */
layer: number | null, };

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

/** The keycodes the remap panel offers, by category. */
export const KEYCODE_CATEGORIES: readonly KeycodeCategory[] = [
  {
    name: "Special",
    keycodes: [
      { keycode: 0x0000, name: "NO", layer: null },
      { keycode: 0x0001, name: "TRNS", layer: null },
    ],
  },
  {
    name: "Letters",
    keycodes: [
      { keycode: 0x0004, name: "A", layer: null },
      { keycode: 0x0005, name: "B", layer: null },
      { keycode: 0x0006, name: "C", layer: null },
      { keycode: 0x0007, name: "D", layer: null },
      { keycode: 0x0008, name: "E", layer: null },
      { keycode: 0x0009, name: "F", layer: null },
      { keycode: 0x000a, name: "G", layer: null },
      { keycode: 0x000b, name: "H", layer: null },
      { keycode: 0x000c, name: "I", layer: null },
      { keycode: 0x000d, name: "J", layer: null },
      { keycode: 0x000e, name: "K", layer: null },
      { keycode: 0x000f, name: "L", layer: null },
      { keycode: 0x0010, name: "M", layer: null },
      { keycode: 0x0011, name: "N", layer: null },
      { keycode: 0x0012, name: "O", layer: null },
      { keycode: 0x0013, name: "P", layer: null },
      { keycode: 0x0014, name: "Q", layer: null },
      { keycode: 0x0015, name: "R", layer: null },
      { keycode: 0x0016, name: "S", layer: null },
      { keycode: 0x0017, name: "T", layer: null },
      { keycode: 0x0018, name: "U", layer: null },
      { keycode: 0x0019, name: "V", layer: null },
      { keycode: 0x001a, name: "W", layer: null },
      { keycode: 0x001b, name: "X", layer: null },
      { keycode: 0x001c, name: "Y", layer: null },
      { keycode: 0x001d, name: "Z", layer: null },
    ],
  },
  {
    name: "Numbers",
    keycodes: [
      { keycode: 0x001e, name: "1", layer: null },
      { keycode: 0x001f, name: "2", layer: null },
      { keycode: 0x0020, name: "3", layer: null },
      { keycode: 0x0021, name: "4", layer: null },
      { keycode: 0x0022, name: "5", layer: null },
      { keycode: 0x0023, name: "6", layer: null },
      { keycode: 0x0024, name: "7", layer: null },
      { keycode: 0x0025, name: "8", layer: null },
      { keycode: 0x0026, name: "9", layer: null },
      { keycode: 0x0027, name: "0", layer: null },
    ],
  },
  {
    name: "Editing",
    keycodes: [
      { keycode: 0x0028, name: "ENT", layer: null },
      { keycode: 0x0029, name: "ESC", layer: null },
      { keycode: 0x002a, name: "BSPC", layer: null },
      { keycode: 0x002b, name: "TAB", layer: null },
      { keycode: 0x002c, name: "SPC", layer: null },
      { keycode: 0x0039, name: "CAPS", layer: null },
      { keycode: 0x0049, name: "INS", layer: null },
      { keycode: 0x004c, name: "DEL", layer: null },
    ],
  },
  {
    name: "Punctuation",
    keycodes: [
      { keycode: 0x002d, name: "MINS", layer: null },
      { keycode: 0x002e, name: "EQL", layer: null },
      { keycode: 0x002f, name: "LBRC", layer: null },
      { keycode: 0x0030, name: "RBRC", layer: null },
      { keycode: 0x0031, name: "BSLS", layer: null },
      { keycode: 0x0032, name: "NUHS", layer: null },
      { keycode: 0x0033, name: "SCLN", layer: null },
      { keycode: 0x0034, name: "QUOT", layer: null },
      { keycode: 0x0035, name: "GRV", layer: null },
      { keycode: 0x0036, name: "COMM", layer: null },
      { keycode: 0x0037, name: "DOT", layer: null },
      { keycode: 0x0038, name: "SLSH", layer: null },
      { keycode: 0x0064, name: "NUBS", layer: null },
    ],
  },
  {
    name: "Navigation",
    keycodes: [
      { keycode: 0x004a, name: "HOME", layer: null },
      { keycode: 0x004b, name: "PGUP", layer: null },
      { keycode: 0x004d, name: "END", layer: null },
      { keycode: 0x004e, name: "PGDN", layer: null },
      { keycode: 0x004f, name: "RGHT", layer: null },
      { keycode: 0x0050, name: "LEFT", layer: null },
      { keycode: 0x0051, name: "DOWN", layer: null },
      { keycode: 0x0052, name: "UP", layer: null },
    ],
  },
  {
    name: "Function keys",
    keycodes: [
      { keycode: 0x003a, name: "F1", layer: null },
      { keycode: 0x003b, name: "F2", layer: null },
      { keycode: 0x003c, name: "F3", layer: null },
      { keycode: 0x003d, name: "F4", layer: null },
      { keycode: 0x003e, name: "F5", layer: null },
      { keycode: 0x003f, name: "F6", layer: null },
      { keycode: 0x0040, name: "F7", layer: null },
      { keycode: 0x0041, name: "F8", layer: null },
      { keycode: 0x0042, name: "F9", layer: null },
      { keycode: 0x0043, name: "F10", layer: null },
      { keycode: 0x0044, name: "F11", layer: null },
      { keycode: 0x0045, name: "F12", layer: null },
      { keycode: 0x0068, name: "F13", layer: null },
      { keycode: 0x0069, name: "F14", layer: null },
      { keycode: 0x006a, name: "F15", layer: null },
      { keycode: 0x006b, name: "F16", layer: null },
      { keycode: 0x006c, name: "F17", layer: null },
      { keycode: 0x006d, name: "F18", layer: null },
      { keycode: 0x006e, name: "F19", layer: null },
      { keycode: 0x006f, name: "F20", layer: null },
      { keycode: 0x0070, name: "F21", layer: null },
      { keycode: 0x0071, name: "F22", layer: null },
      { keycode: 0x0072, name: "F23", layer: null },
      { keycode: 0x0073, name: "F24", layer: null },
    ],
  },
  {
    name: "Modifiers",
    keycodes: [
      { keycode: 0x00e0, name: "LCTL", layer: null },
      { keycode: 0x00e1, name: "LSFT", layer: null },
      { keycode: 0x00e2, name: "LALT", layer: null },
      { keycode: 0x00e3, name: "LGUI", layer: null },
      { keycode: 0x00e4, name: "RCTL", layer: null },
      { keycode: 0x00e5, name: "RSFT", layer: null },
      { keycode: 0x00e6, name: "RALT", layer: null },
      { keycode: 0x00e7, name: "RGUI", layer: null },
    ],
  },
  {
    name: "Layers",
    keycodes: [
      { keycode: 0x5200, name: "TO(0)", layer: 0 },
      { keycode: 0x5201, name: "TO(1)", layer: 1 },
      { keycode: 0x5202, name: "TO(2)", layer: 2 },
      { keycode: 0x5203, name: "TO(3)", layer: 3 },
      { keycode: 0x5204, name: "TO(4)", layer: 4 },
      { keycode: 0x5205, name: "TO(5)", layer: 5 },
      { keycode: 0x5206, name: "TO(6)", layer: 6 },
      { keycode: 0x5207, name: "TO(7)", layer: 7 },
      { keycode: 0x5208, name: "TO(8)", layer: 8 },
      { keycode: 0x5209, name: "TO(9)", layer: 9 },
      { keycode: 0x520a, name: "TO(10)", layer: 10 },
      { keycode: 0x520b, name: "TO(11)", layer: 11 },
      { keycode: 0x520c, name: "TO(12)", layer: 12 },
      { keycode: 0x520d, name: "TO(13)", layer: 13 },
      { keycode: 0x520e, name: "TO(14)", layer: 14 },
      { keycode: 0x520f, name: "TO(15)", layer: 15 },
      { keycode: 0x5210, name: "TO(16)", layer: 16 },
      { keycode: 0x5211, name: "TO(17)", layer: 17 },
      { keycode: 0x5212, name: "TO(18)", layer: 18 },
      { keycode: 0x5213, name: "TO(19)", layer: 19 },
      { keycode: 0x5214, name: "TO(20)", layer: 20 },
      { keycode: 0x5215, name: "TO(21)", layer: 21 },
      { keycode: 0x5216, name: "TO(22)", layer: 22 },
      { keycode: 0x5217, name: "TO(23)", layer: 23 },
      { keycode: 0x5218, name: "TO(24)", layer: 24 },
      { keycode: 0x5219, name: "TO(25)", layer: 25 },
      { keycode: 0x521a, name: "TO(26)", layer: 26 },
      { keycode: 0x521b, name: "TO(27)", layer: 27 },
      { keycode: 0x521c, name: "TO(28)", layer: 28 },
      { keycode: 0x521d, name: "TO(29)", layer: 29 },
      { keycode: 0x521e, name: "TO(30)", layer: 30 },
      { keycode: 0x521f, name: "TO(31)", layer: 31 },
      { keycode: 0x5220, name: "MO(0)", layer: 0 },
      { keycode: 0x5221, name: "MO(1)", layer: 1 },
      { keycode: 0x5222, name: "MO(2)", layer: 2 },
      { keycode: 0x5223, name: "MO(3)", layer: 3 },
      { keycode: 0x5224, name: "MO(4)", layer: 4 },
      { keycode: 0x5225, name: "MO(5)", layer: 5 },
      { keycode: 0x5226, name: "MO(6)", layer: 6 },
      { keycode: 0x5227, name: "MO(7)", layer: 7 },
      { keycode: 0x5228, name: "MO(8)", layer: 8 },
      { keycode: 0x5229, name: "MO(9)", layer: 9 },
      { keycode: 0x522a, name: "MO(10)", layer: 10 },
      { keycode: 0x522b, name: "MO(11)", layer: 11 },
      { keycode: 0x522c, name: "MO(12)", layer: 12 },
      { keycode: 0x522d, name: "MO(13)", layer: 13 },
      { keycode: 0x522e, name: "MO(14)", layer: 14 },
      { keycode: 0x522f, name: "MO(15)", layer: 15 },
      { keycode: 0x5230, name: "MO(16)", layer: 16 },
      { keycode: 0x5231, name: "MO(17)", layer: 17 },
      { keycode: 0x5232, name: "MO(18)", layer: 18 },
      { keycode: 0x5233, name: "MO(19)", layer: 19 },
      { keycode: 0x5234, name: "MO(20)", layer: 20 },
      { keycode: 0x5235, name: "MO(21)", layer: 21 },
      { keycode: 0x5236, name: "MO(22)", layer: 22 },
      { keycode: 0x5237, name: "MO(23)", layer: 23 },
      { keycode: 0x5238, name: "MO(24)", layer: 24 },
      { keycode: 0x5239, name: "MO(25)", layer: 25 },
      { keycode: 0x523a, name: "MO(26)", layer: 26 },
      { keycode: 0x523b, name: "MO(27)", layer: 27 },
      { keycode: 0x523c, name: "MO(28)", layer: 28 },
      { keycode: 0x523d, name: "MO(29)", layer: 29 },
      { keycode: 0x523e, name: "MO(30)", layer: 30 },
      { keycode: 0x523f, name: "MO(31)", layer: 31 },
      { keycode: 0x5240, name: "DF(0)", layer: 0 },
      { keycode: 0x5241, name: "DF(1)", layer: 1 },
      { keycode: 0x5242, name: "DF(2)", layer: 2 },
      { keycode: 0x5243, name: "DF(3)", layer: 3 },
      { keycode: 0x5244, name: "DF(4)", layer: 4 },
      { keycode: 0x5245, name: "DF(5)", layer: 5 },
      { keycode: 0x5246, name: "DF(6)", layer: 6 },
      { keycode: 0x5247, name: "DF(7)", layer: 7 },
      { keycode: 0x5248, name: "DF(8)", layer: 8 },
      { keycode: 0x5249, name: "DF(9)", layer: 9 },
      { keycode: 0x524a, name: "DF(10)", layer: 10 },
      { keycode: 0x524b, name: "DF(11)", layer: 11 },
      { keycode: 0x524c, name: "DF(12)", layer: 12 },
      { keycode: 0x524d, name: "DF(13)", layer: 13 },
      { keycode: 0x524e, name: "DF(14)", layer: 14 },
      { keycode: 0x524f, name: "DF(15)", layer: 15 },
      { keycode: 0x5250, name: "DF(16)", layer: 16 },
      { keycode: 0x5251, name: "DF(17)", layer: 17 },
      { keycode: 0x5252, name: "DF(18)", layer: 18 },
      { keycode: 0x5253, name: "DF(19)", layer: 19 },
      { keycode: 0x5254, name: "DF(20)", layer: 20 },
      { keycode: 0x5255, name: "DF(21)", layer: 21 },
      { keycode: 0x5256, name: "DF(22)", layer: 22 },
      { keycode: 0x5257, name: "DF(23)", layer: 23 },
      { keycode: 0x5258, name: "DF(24)", layer: 24 },
      { keycode: 0x5259, name: "DF(25)", layer: 25 },
      { keycode: 0x525a, name: "DF(26)", layer: 26 },
      { keycode: 0x525b, name: "DF(27)", layer: 27 },
      { keycode: 0x525c, name: "DF(28)", layer: 28 },
      { keycode: 0x525d, name: "DF(29)", layer: 29 },
      { keycode: 0x525e, name: "DF(30)", layer: 30 },
      { keycode: 0x525f, name: "DF(31)", layer: 31 },
      { keycode: 0x5260, name: "TG(0)", layer: 0 },
      { keycode: 0x5261, name: "TG(1)", layer: 1 },
      { keycode: 0x5262, name: "TG(2)", layer: 2 },
      { keycode: 0x5263, name: "TG(3)", layer: 3 },
      { keycode: 0x5264, name: "TG(4)", layer: 4 },
      { keycode: 0x5265, name: "TG(5)", layer: 5 },
      { keycode: 0x5266, name: "TG(6)", layer: 6 },
      { keycode: 0x5267, name: "TG(7)", layer: 7 },
      { keycode: 0x5268, name: "TG(8)", layer: 8 },
      { keycode: 0x5269, name: "TG(9)", layer: 9 },
      { keycode: 0x526a, name: "TG(10)", layer: 10 },
      { keycode: 0x526b, name: "TG(11)", layer: 11 },
      { keycode: 0x526c, name: "TG(12)", layer: 12 },
      { keycode: 0x526d, name: "TG(13)", layer: 13 },
      { keycode: 0x526e, name: "TG(14)", layer: 14 },
      { keycode: 0x526f, name: "TG(15)", layer: 15 },
      { keycode: 0x5270, name: "TG(16)", layer: 16 },
      { keycode: 0x5271, name: "TG(17)", layer: 17 },
      { keycode: 0x5272, name: "TG(18)", layer: 18 },
      { keycode: 0x5273, name: "TG(19)", layer: 19 },
      { keycode: 0x5274, name: "TG(20)", layer: 20 },
      { keycode: 0x5275, name: "TG(21)", layer: 21 },
      { keycode: 0x5276, name: "TG(22)", layer: 22 },
      { keycode: 0x5277, name: "TG(23)", layer: 23 },
      { keycode: 0x5278, name: "TG(24)", layer: 24 },
      { keycode: 0x5279, name: "TG(25)", layer: 25 },
      { keycode: 0x527a, name: "TG(26)", layer: 26 },
      { keycode: 0x527b, name: "TG(27)", layer: 27 },
      { keycode: 0x527c, name: "TG(28)", layer: 28 },
      { keycode: 0x527d, name: "TG(29)", layer: 29 },
      { keycode: 0x527e, name: "TG(30)", layer: 30 },
      { keycode: 0x527f, name: "TG(31)", layer: 31 },
      { keycode: 0x5280, name: "OSL(0)", layer: 0 },
      { keycode: 0x5281, name: "OSL(1)", layer: 1 },
      { keycode: 0x5282, name: "OSL(2)", layer: 2 },
      { keycode: 0x5283, name: "OSL(3)", layer: 3 },
      { keycode: 0x5284, name: "OSL(4)", layer: 4 },
      { keycode: 0x5285, name: "OSL(5)", layer: 5 },
      { keycode: 0x5286, name: "OSL(6)", layer: 6 },
      { keycode: 0x5287, name: "OSL(7)", layer: 7 },
      { keycode: 0x5288, name: "OSL(8)", layer: 8 },
      { keycode: 0x5289, name: "OSL(9)", layer: 9 },
      { keycode: 0x528a, name: "OSL(10)", layer: 10 },
      { keycode: 0x528b, name: "OSL(11)", layer: 11 },
      { keycode: 0x528c, name: "OSL(12)", layer: 12 },
      { keycode: 0x528d, name: "OSL(13)", layer: 13 },
      { keycode: 0x528e, name: "OSL(14)", layer: 14 },
      { keycode: 0x528f, name: "OSL(15)", layer: 15 },
      { keycode: 0x5290, name: "OSL(16)", layer: 16 },
      { keycode: 0x5291, name: "OSL(17)", layer: 17 },
      { keycode: 0x5292, name: "OSL(18)", layer: 18 },
      { keycode: 0x5293, name: "OSL(19)", layer: 19 },
      { keycode: 0x5294, name: "OSL(20)", layer: 20 },
      { keycode: 0x5295, name: "OSL(21)", layer: 21 },
      { keycode: 0x5296, name: "OSL(22)", layer: 22 },
      { keycode: 0x5297, name: "OSL(23)", layer: 23 },
      { keycode: 0x5298, name: "OSL(24)", layer: 24 },
      { keycode: 0x5299, name: "OSL(25)", layer: 25 },
      { keycode: 0x529a, name: "OSL(26)", layer: 26 },
      { keycode: 0x529b, name: "OSL(27)", layer: 27 },
      { keycode: 0x529c, name: "OSL(28)", layer: 28 },
      { keycode: 0x529d, name: "OSL(29)", layer: 29 },
      { keycode: 0x529e, name: "OSL(30)", layer: 30 },
      { keycode: 0x529f, name: "OSL(31)", layer: 31 },
      { keycode: 0x52c0, name: "TT(0)", layer: 0 },
      { keycode: 0x52c1, name: "TT(1)", layer: 1 },
      { keycode: 0x52c2, name: "TT(2)", layer: 2 },
      { keycode: 0x52c3, name: "TT(3)", layer: 3 },
      { keycode: 0x52c4, name: "TT(4)", layer: 4 },
      { keycode: 0x52c5, name: "TT(5)", layer: 5 },
      { keycode: 0x52c6, name: "TT(6)", layer: 6 },
      { keycode: 0x52c7, name: "TT(7)", layer: 7 },
      { keycode: 0x52c8, name: "TT(8)", layer: 8 },
      { keycode: 0x52c9, name: "TT(9)", layer: 9 },
      { keycode: 0x52ca, name: "TT(10)", layer: 10 },
      { keycode: 0x52cb, name: "TT(11)", layer: 11 },
      { keycode: 0x52cc, name: "TT(12)", layer: 12 },
      { keycode: 0x52cd, name: "TT(13)", layer: 13 },
      { keycode: 0x52ce, name: "TT(14)", layer: 14 },
      { keycode: 0x52cf, name: "TT(15)", layer: 15 },
      { keycode: 0x52d0, name: "TT(16)", layer: 16 },
      { keycode: 0x52d1, name: "TT(17)", layer: 17 },
      { keycode: 0x52d2, name: "TT(18)", layer: 18 },
      { keycode: 0x52d3, name: "TT(19)", layer: 19 },
      { keycode: 0x52d4, name: "TT(20)", layer: 20 },
      { keycode: 0x52d5, name: "TT(21)", layer: 21 },
      { keycode: 0x52d6, name: "TT(22)", layer: 22 },
      { keycode: 0x52d7, name: "TT(23)", layer: 23 },
      { keycode: 0x52d8, name: "TT(24)", layer: 24 },
      { keycode: 0x52d9, name: "TT(25)", layer: 25 },
      { keycode: 0x52da, name: "TT(26)", layer: 26 },
      { keycode: 0x52db, name: "TT(27)", layer: 27 },
      { keycode: 0x52dc, name: "TT(28)", layer: 28 },
      { keycode: 0x52dd, name: "TT(29)", layer: 29 },
      { keycode: 0x52de, name: "TT(30)", layer: 30 },
      { keycode: 0x52df, name: "TT(31)", layer: 31 },
      { keycode: 0x52e0, name: "PDF(0)", layer: 0 },
      { keycode: 0x52e1, name: "PDF(1)", layer: 1 },
      { keycode: 0x52e2, name: "PDF(2)", layer: 2 },
      { keycode: 0x52e3, name: "PDF(3)", layer: 3 },
      { keycode: 0x52e4, name: "PDF(4)", layer: 4 },
      { keycode: 0x52e5, name: "PDF(5)", layer: 5 },
      { keycode: 0x52e6, name: "PDF(6)", layer: 6 },
      { keycode: 0x52e7, name: "PDF(7)", layer: 7 },
      { keycode: 0x52e8, name: "PDF(8)", layer: 8 },
      { keycode: 0x52e9, name: "PDF(9)", layer: 9 },
      { keycode: 0x52ea, name: "PDF(10)", layer: 10 },
      { keycode: 0x52eb, name: "PDF(11)", layer: 11 },
      { keycode: 0x52ec, name: "PDF(12)", layer: 12 },
      { keycode: 0x52ed, name: "PDF(13)", layer: 13 },
      { keycode: 0x52ee, name: "PDF(14)", layer: 14 },
      { keycode: 0x52ef, name: "PDF(15)", layer: 15 },
      { keycode: 0x52f0, name: "PDF(16)", layer: 16 },
      { keycode: 0x52f1, name: "PDF(17)", layer: 17 },
      { keycode: 0x52f2, name: "PDF(18)", layer: 18 },
      { keycode: 0x52f3, name: "PDF(19)", layer: 19 },
      { keycode: 0x52f4, name: "PDF(20)", layer: 20 },
      { keycode: 0x52f5, name: "PDF(21)", layer: 21 },
      { keycode: 0x52f6, name: "PDF(22)", layer: 22 },
      { keycode: 0x52f7, name: "PDF(23)", layer: 23 },
      { keycode: 0x52f8, name: "PDF(24)", layer: 24 },
      { keycode: 0x52f9, name: "PDF(25)", layer: 25 },
      { keycode: 0x52fa, name: "PDF(26)", layer: 26 },
      { keycode: 0x52fb, name: "PDF(27)", layer: 27 },
      { keycode: 0x52fc, name: "PDF(28)", layer: 28 },
      { keycode: 0x52fd, name: "PDF(29)", layer: 29 },
      { keycode: 0x52fe, name: "PDF(30)", layer: 30 },
      { keycode: 0x52ff, name: "PDF(31)", layer: 31 },
    ],
  },
  {
    name: "Numpad",
    keycodes: [
      { keycode: 0x0053, name: "NUM", layer: null },
      { keycode: 0x0054, name: "PSLS", layer: null },
      { keycode: 0x0055, name: "PAST", layer: null },
      { keycode: 0x0056, name: "PMNS", layer: null },
      { keycode: 0x0057, name: "PPLS", layer: null },
      { keycode: 0x0058, name: "PENT", layer: null },
      { keycode: 0x0059, name: "P1", layer: null },
      { keycode: 0x005a, name: "P2", layer: null },
      { keycode: 0x005b, name: "P3", layer: null },
      { keycode: 0x005c, name: "P4", layer: null },
      { keycode: 0x005d, name: "P5", layer: null },
      { keycode: 0x005e, name: "P6", layer: null },
      { keycode: 0x005f, name: "P7", layer: null },
      { keycode: 0x0060, name: "P8", layer: null },
      { keycode: 0x0061, name: "P9", layer: null },
      { keycode: 0x0062, name: "P0", layer: null },
      { keycode: 0x0063, name: "PDOT", layer: null },
      { keycode: 0x0067, name: "PEQL", layer: null },
      { keycode: 0x0085, name: "PCMM", layer: null },
      { keycode: 0x0086, name: "KP_EQUAL_AS400", layer: null },
    ],
  },
  {
    name: "Media and system",
    keycodes: [
      { keycode: 0x0066, name: "KB_POWER", layer: null },
      { keycode: 0x007f, name: "KB_MUTE", layer: null },
      { keycode: 0x0080, name: "KB_VOLUME_UP", layer: null },
      { keycode: 0x0081, name: "KB_VOLUME_DOWN", layer: null },
      { keycode: 0x00a5, name: "PWR", layer: null },
      { keycode: 0x00a6, name: "SLEP", layer: null },
      { keycode: 0x00a7, name: "WAKE", layer: null },
      { keycode: 0x00a8, name: "MUTE", layer: null },
      { keycode: 0x00a9, name: "VOLU", layer: null },
      { keycode: 0x00aa, name: "VOLD", layer: null },
      { keycode: 0x00ab, name: "MNXT", layer: null },
      { keycode: 0x00ac, name: "MPRV", layer: null },
      { keycode: 0x00ad, name: "MSTP", layer: null },
      { keycode: 0x00ae, name: "MPLY", layer: null },
      { keycode: 0x00af, name: "MSEL", layer: null },
      { keycode: 0x00b0, name: "EJCT", layer: null },
      { keycode: 0x00b1, name: "MAIL", layer: null },
      { keycode: 0x00b2, name: "CALC", layer: null },
      { keycode: 0x00b3, name: "MYCM", layer: null },
      { keycode: 0x00b4, name: "WSCH", layer: null },
      { keycode: 0x00b5, name: "WHOM", layer: null },
      { keycode: 0x00b6, name: "WBAK", layer: null },
      { keycode: 0x00b7, name: "WFWD", layer: null },
      { keycode: 0x00b8, name: "WSTP", layer: null },
      { keycode: 0x00b9, name: "WREF", layer: null },
      { keycode: 0x00ba, name: "WFAV", layer: null },
      { keycode: 0x00bb, name: "MFFD", layer: null },
      { keycode: 0x00bc, name: "MRWD", layer: null },
      { keycode: 0x00bd, name: "BRIU", layer: null },
      { keycode: 0x00be, name: "BRID", layer: null },
      { keycode: 0x00bf, name: "CPNL", layer: null },
      { keycode: 0x00c0, name: "ASST", layer: null },
      { keycode: 0x00c1, name: "MCTL", layer: null },
      { keycode: 0x00c2, name: "LPAD", layer: null },
    ],
  },
  {
    name: "Mouse",
    keycodes: [
      { keycode: 0x00cd, name: "MS_UP", layer: null },
      { keycode: 0x00ce, name: "MS_DOWN", layer: null },
      { keycode: 0x00cf, name: "MS_LEFT", layer: null },
      { keycode: 0x00d0, name: "MS_RGHT", layer: null },
      { keycode: 0x00d1, name: "MS_BTN1", layer: null },
      { keycode: 0x00d2, name: "MS_BTN2", layer: null },
      { keycode: 0x00d3, name: "MS_BTN3", layer: null },
      { keycode: 0x00d4, name: "MS_BTN4", layer: null },
      { keycode: 0x00d5, name: "MS_BTN5", layer: null },
      { keycode: 0x00d6, name: "MS_BTN6", layer: null },
      { keycode: 0x00d7, name: "MS_BTN7", layer: null },
      { keycode: 0x00d8, name: "MS_BTN8", layer: null },
      { keycode: 0x00d9, name: "MS_WHLU", layer: null },
      { keycode: 0x00da, name: "MS_WHLD", layer: null },
      { keycode: 0x00db, name: "MS_WHLL", layer: null },
      { keycode: 0x00dc, name: "MS_WHLR", layer: null },
      { keycode: 0x00dd, name: "MS_ACL0", layer: null },
      { keycode: 0x00de, name: "MS_ACL1", layer: null },
      { keycode: 0x00df, name: "MS_ACL2", layer: null },
    ],
  },
  {
    name: "International",
    keycodes: [
      { keycode: 0x0087, name: "INT1", layer: null },
      { keycode: 0x0088, name: "INT2", layer: null },
      { keycode: 0x0089, name: "INT3", layer: null },
      { keycode: 0x008a, name: "INT4", layer: null },
      { keycode: 0x008b, name: "INT5", layer: null },
      { keycode: 0x008c, name: "INT6", layer: null },
      { keycode: 0x008d, name: "INT7", layer: null },
      { keycode: 0x008e, name: "INT8", layer: null },
      { keycode: 0x008f, name: "INT9", layer: null },
      { keycode: 0x0090, name: "LNG1", layer: null },
      { keycode: 0x0091, name: "LNG2", layer: null },
      { keycode: 0x0092, name: "LNG3", layer: null },
      { keycode: 0x0093, name: "LNG4", layer: null },
      { keycode: 0x0094, name: "LNG5", layer: null },
      { keycode: 0x0095, name: "LNG6", layer: null },
      { keycode: 0x0096, name: "LNG7", layer: null },
      { keycode: 0x0097, name: "LNG8", layer: null },
      { keycode: 0x0098, name: "LNG9", layer: null },
    ],
  },
  {
    name: "Other",
    keycodes: [
      { keycode: 0x0046, name: "PSCR", layer: null },
      { keycode: 0x0047, name: "SCRL", layer: null },
      { keycode: 0x0048, name: "PAUS", layer: null },
      { keycode: 0x0065, name: "APP", layer: null },
      { keycode: 0x0074, name: "EXEC", layer: null },
      { keycode: 0x0075, name: "HELP", layer: null },
      { keycode: 0x0076, name: "MENU", layer: null },
      { keycode: 0x0077, name: "SLCT", layer: null },
      { keycode: 0x0078, name: "STOP", layer: null },
      { keycode: 0x0079, name: "AGIN", layer: null },
      { keycode: 0x007a, name: "UNDO", layer: null },
      { keycode: 0x007b, name: "CUT", layer: null },
      { keycode: 0x007c, name: "COPY", layer: null },
      { keycode: 0x007d, name: "PSTE", layer: null },
      { keycode: 0x007e, name: "FIND", layer: null },
      { keycode: 0x0082, name: "LCAP", layer: null },
      { keycode: 0x0083, name: "LNUM", layer: null },
      { keycode: 0x0084, name: "LSCR", layer: null },
      { keycode: 0x0099, name: "ERAS", layer: null },
      { keycode: 0x009a, name: "SYRQ", layer: null },
      { keycode: 0x009b, name: "CNCL", layer: null },
      { keycode: 0x009c, name: "CLR", layer: null },
      { keycode: 0x009d, name: "PRIR", layer: null },
      { keycode: 0x009e, name: "RETN", layer: null },
      { keycode: 0x009f, name: "SEPR", layer: null },
      { keycode: 0x00a0, name: "OUT", layer: null },
      { keycode: 0x00a1, name: "OPER", layer: null },
      { keycode: 0x00a2, name: "CLAG", layer: null },
      { keycode: 0x00a3, name: "CRSL", layer: null },
      { keycode: 0x00a4, name: "EXSL", layer: null },
    ],
  },
];
