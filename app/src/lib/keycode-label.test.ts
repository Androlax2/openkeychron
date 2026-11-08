import { describe, expect, it } from "vitest";
import { fitKeycodeLabel, shortenKeycodeName } from "./keycode-label";

describe("shortenKeycodeName", () => {
  it.each([
    ["MT(MOD_LCTL | MOD_LSFT | MOD_LALT, A)", "MEH_T(A)"],
    ["MT(MOD_LCTL | MOD_LSFT | MOD_LALT | MOD_LGUI, A)", "HYPR_T(A)"],
    ["LCTL(LSFT(A))", "LCS(A)"],
    ["LCTL(LSFT(LALT(A)))", "MEH(A)"],
    ["LCTL(LSFT(LALT(LGUI(A))))", "HYPR(A)"],
    ["LGUI(LALT(LSFT(LCTL(A))))", "HYPR(A)"],
  ])("writes %s as %s", (name, shorthand) => {
    expect(shortenKeycodeName(name)).toBe(shorthand);
  });

  it.each([
    "A",
    "LCTL(C)",
    "LT(1, SPC)",
    "MT(MOD_LCTL | MOD_LGUI, A)",
    "MT(MOD_LCTL | MOD_LSFT, A)",
    "LCTL(LGUI(A))",
  ])("leaves %s unchanged", (name) => {
    expect(shortenKeycodeName(name)).toBe(name);
  });
});

describe("fitKeycodeLabel", () => {
  it("keeps a label that fits at its normal size", () => {
    expect(fitKeycodeLabel("Q", 40)).toEqual({ text: "Q", fontSize: null });
  });

  it("shrinks a label that is too wide", () => {
    expect(fitKeycodeLabel("LT(1, SPC)", 40)).toEqual({ text: "LT(1, SPC)", fontSize: 6 });
  });

  it("stops shrinking at a readable size and ends the label with an ellipsis", () => {
    const fitted = fitKeycodeLabel("MT(MOD_LCTL | MOD_LGUI, A)", 40);
    expect(fitted.fontSize).toBe(6);
    expect(fitted.text.endsWith("…")).toBe(true);
    expect(fitted.text.length).toBeLessThan(26);
  });
});
