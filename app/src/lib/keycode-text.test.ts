import { describe, expect, it } from "vitest";
import { parseRawKeycode } from "./keycode-text";

describe("parseRawKeycode", () => {
  it.each([
    ["0x0106", 0x0106],
    ["0X7E05", 0x7e05],
    ["106", 0x0106],
    ["  ffff ", 0xffff],
    ["0", 0],
  ])("reads %s as hexadecimal", (text, keycode) => {
    expect(parseRawKeycode(text)).toBe(keycode);
  });

  it.each([[""], ["0x"], ["12345"], ["0x1g"], ["-1"], ["1.5"]])("rejects %j", (text) => {
    expect(parseRawKeycode(text)).toBeNull();
  });
});
