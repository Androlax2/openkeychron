import type { Key } from "./bindings";

export function hex(keycode: number): string {
  return `0x${keycode.toString(16).padStart(4, "0")}`;
}

/** QMK's name with the raw value, or only the raw value of a keycode without a name. */
export function describeKeycode(key: Pick<Key, "keycode" | "name">): string {
  return key.name === null ? hex(key.keycode) : `${key.name} (${hex(key.keycode)})`;
}
