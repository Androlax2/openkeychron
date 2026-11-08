import type { KeycodeCategory } from "./bindings";
import { hex } from "./keycode-text";

/**
 * The categories and keycodes the picker shows for `search`: a keycode whose name or
 * hexadecimal value contains it, or every keycode of a category whose name contains it,
 * ignoring case. Layer keycodes for a layer beyond `layerCount` are never shown, and neither is
 * a category left empty.
 */
export function matchingCategories(
  categories: readonly KeycodeCategory[],
  search: string,
  layerCount: number,
): KeycodeCategory[] {
  const wanted = search.trim().toLowerCase();
  const contains = (text: string) => text.toLowerCase().includes(wanted);
  return categories
    .map((category) => ({
      name: category.name,
      keycodes: category.keycodes.filter(
        (choice) =>
          (choice.layer === null || choice.layer < layerCount) &&
          (contains(category.name) || contains(choice.name) || contains(hex(choice.keycode))),
      ),
    }))
    .filter((category) => category.keycodes.length > 0);
}
