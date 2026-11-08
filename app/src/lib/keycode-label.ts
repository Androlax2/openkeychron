const NORMAL_FONT_SIZE = 9;
// Below this a label is unreadable, so a longer one is cut with an ellipsis instead.
const MINIMUM_FONT_SIZE = 6;
// A generous width for one character at the normal size, so a name estimated to fit does fit.
const CHARACTER_WIDTH_AT_NORMAL_SIZE = 6;

const MODIFIER_WRAPPERS = ["LCTL", "LSFT", "LALT", "LGUI"];

// QMK's shorthands from quantum/keycodes.h. Ctrl+Shift has a wrapper (LCS) but no mod-tap
// shorthand that is verified here, so its mod-tap keeps the MT(...) name., keyed by the sorted left-hand modifiers.
const MODIFIER_SHORTHANDS: Record<string, { wrapper: string; modTap: string | null }> = {
  "LCTL,LSFT": { wrapper: "LCS", modTap: null },
  "LALT,LCTL,LSFT": { wrapper: "MEH", modTap: "MEH_T" },
  "LALT,LCTL,LGUI,LSFT": { wrapper: "HYPR", modTap: "HYPR_T" },
};

const MOD_TAP_PATTERN = /^MT\(((?:MOD_\w+)(?: \| MOD_\w+)*), (.+)\)$/;
const WRAPPER_PATTERN = new RegExp(`^(${MODIFIER_WRAPPERS.join("|")})\\((.+)\\)$`);

function shorthandFor(modifiers: string[]) {
  return MODIFIER_SHORTHANDS[[...modifiers].sort().join(",")];
}

/**
 * QMK's shorthand for a long keycode name, such as `MEH_T(A)` for
 * `MT(MOD_LCTL | MOD_LSFT | MOD_LALT, A)` and `HYPR(A)` for `LCTL(LSFT(LALT(LGUI(A))))`; the name itself
 * when QMK has no shorthand for it.
 */
export function shortenKeycodeName(name: string): string {
  const modTap = MOD_TAP_PATTERN.exec(name);
  if (modTap !== null) {
    const [, modifierList = "", key = ""] = modTap;
    const shorthand = shorthandFor(modifierList.split(" | ").map((modifier) => modifier.slice(4)));
    return shorthand?.modTap == null ? name : `${shorthand.modTap}(${key})`;
  }

  const modifiers: string[] = [];
  let inner = name;
  for (
    let wrapped = WRAPPER_PATTERN.exec(inner);
    wrapped !== null;
    wrapped = WRAPPER_PATTERN.exec(inner)
  ) {
    modifiers.push(wrapped[1] ?? "");
    inner = wrapped[2] ?? "";
  }
  const shorthand = shorthandFor(modifiers);
  return shorthand === undefined ? name : `${shorthand.wrapper}(${inner})`;
}

export type FittedLabel = {
  text: string;
  /** The size to use in pixels, or null for the stylesheet's size. */
  fontSize: number | null;
};

/**
 * A keycode label that fits `availableWidth` pixels: shrunk down to a readable minimum, then
 * cut with an ellipsis. Shrinking, unlike SVG's textLength, never stretches a label the
 * estimate made too wide.
 */
export function fitKeycodeLabel(label: string, availableWidth: number): FittedLabel {
  const estimatedWidth = label.length * CHARACTER_WIDTH_AT_NORMAL_SIZE;
  if (estimatedWidth <= availableWidth) {
    return { text: label, fontSize: null };
  }
  const fontSize = (NORMAL_FONT_SIZE * availableWidth) / estimatedWidth;
  if (fontSize >= MINIMUM_FONT_SIZE) {
    return { text: label, fontSize };
  }
  const characterWidth = (CHARACTER_WIDTH_AT_NORMAL_SIZE * MINIMUM_FONT_SIZE) / NORMAL_FONT_SIZE;
  const fittingCharacters = Math.floor(availableWidth / characterWidth);
  return {
    text: `${label.slice(0, Math.max(0, fittingCharacters - 1))}…`,
    fontSize: MINIMUM_FONT_SIZE,
  };
}
