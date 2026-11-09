import type { Key } from "./bindings";

export function hex(keycode: number): string {
  return `0x${keycode.toString(16).padStart(4, "0")}`;
}

/** QMK's name with the raw value, or only the raw value of a keycode without a name. */
export function describeKeycode(key: Pick<Key, "keycode" | "name">): string {
  return key.name === null ? hex(key.keycode) : `${key.name} (${hex(key.keycode)})`;
}

const RAW_KEYCODE = /^(?:0x)?([0-9a-f]{1,4})$/i;

/** A keycode typed as one to four hexadecimal digits, `0x` optional; null for anything else. */
export function parseRawKeycode(text: string): number | null {
  const digits = RAW_KEYCODE.exec(text.trim())?.[1];
  return digits === undefined ? null : Number.parseInt(digits, 16);
}
