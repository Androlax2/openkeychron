// Two-part keycodes from the parts the Rust side generates into bindings.ts: a built keycode is
// its wrapper's bits with the inner part's value, named by the wrapper's text around the inner
// part's name (see KeycodeWrapperChoice). keycode_catalog.rs checks that every keycode built
// this way gets the value and name the Rust crates give it.
import type {
  KeycodeCategory,
  KeycodeWrapperChoice,
  ModifierChoice,
  WrapperBuilder,
} from "./bindings";
import {
  KEYCODE_CATEGORIES,
  KEYCODE_WRAPPERS,
  MODIFIER_CHOICES,
  NAMED_ONLY_KEYCODES,
} from "./bindings";

/** The list's basic keycodes, the keys a modifier combination, mod-tap or layer-tap wraps. */
export const BASIC_CATEGORIES: readonly KeycodeCategory[] = KEYCODE_CATEGORIES.map((category) => ({
  name: category.name,
  keycodes: category.keycodes.filter((choice) => choice.layer === null),
})).filter((category) => category.keycodes.length > 0);

export type TickedModifiers = Pick<ModifierChoice, "ctrl" | "shift" | "alt" | "gui" | "rightHand">;

export type KeycodeParts = {
  modifiers: TickedModifiers;
  layer: number | null;
  /** A basic keycode. */
  key: number | null;
};

function modifierChoice(ticked: TickedModifiers): ModifierChoice | null {
  return (
    MODIFIER_CHOICES.find(
      (choice) =>
        choice.ctrl === ticked.ctrl &&
        choice.shift === ticked.shift &&
        choice.alt === ticked.alt &&
        choice.gui === ticked.gui &&
        choice.rightHand === ticked.rightHand,
    ) ?? null
  );
}

/**
 * The keycode `builder` builds from `parts`, or null while a part it takes is missing: no
 * modifier ticked, no layer or a layer past the wrappers', or no key.
 */
export function buildKeycode(builder: WrapperBuilder, parts: KeycodeParts): number | null {
  const modifiers = modifierChoice(parts.modifiers)?.modifiers ?? null;
  const wrapper = KEYCODE_WRAPPERS.find(
    (candidate) =>
      candidate.builder === builder &&
      (candidate.modifiers === null
        ? candidate.layer === parts.layer
        : candidate.modifiers === modifiers),
  );
  const inner = builder === "layerMod" ? modifiers : parts.key;
  return wrapper === undefined || inner === null ? null : wrapper.keycode | inner;
}

type InnerPart = { value: number; name: string };

function innerParts(wrapper: KeycodeWrapperChoice): InnerPart[] {
  return wrapper.builder === "layerMod"
    ? MODIFIER_CHOICES.map((choice) => ({ value: choice.modifiers, name: choice.name }))
    : BASIC_CATEGORIES.flatMap((category) =>
        category.keycodes.map((choice) => ({ value: choice.keycode, name: choice.name })),
      );
}

let names: Map<number, string> | null = null;

/** Every keycode the app can name before the keyboard reads it back, built on first use. */
function knownNames(): Map<number, string> {
  if (names !== null) {
    return names;
  }
  const known = new Map<number, string>();
  for (const choice of KEYCODE_CATEGORIES.flatMap((category) => category.keycodes)) {
    known.set(choice.keycode, choice.name);
  }
  for (const choice of NAMED_ONLY_KEYCODES) {
    known.set(choice.keycode, choice.name);
  }
  for (const wrapper of KEYCODE_WRAPPERS) {
    for (const inner of innerParts(wrapper)) {
      known.set(wrapper.keycode | inner.value, `${wrapper.before}${inner.name}${wrapper.after}`);
    }
  }
  names = known;
  return known;
}

/**
 * QMK's name for `keycode`, as `Keycode::name` gives it; null when QMK names it not. A Keychron
 * custom keycode is named by the connected model only once the keyboard reads it back.
 */
export function keycodeName(keycode: number): string | null {
  return knownNames().get(keycode) ?? null;
}
