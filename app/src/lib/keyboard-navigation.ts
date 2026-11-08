import type { Bounds } from "./bindings";

export type Direction = "left" | "right" | "up" | "down";

const DIRECTIONS: Record<string, Direction> = {
  ArrowLeft: "left",
  ArrowRight: "right",
  ArrowUp: "up",
  ArrowDown: "down",
};

/** The direction an arrow key names, or null for any other key. */
export function arrowDirection(keyName: string): Direction | null {
  return DIRECTIONS[keyName] ?? null;
}

function centre(bounds: Bounds): { x: number; y: number } {
  return { x: bounds.x + bounds.width / 2, y: bounds.y + bounds.height / 2 };
}

/**
 * The key an arrow press moves to: the nearest key whose centre lies in that direction,
 * with distance across the direction counting double so the move stays on its row or column.
 * A tie goes to the key that comes first in `keys`. Null when no key lies that way.
 */
export function neighbour<Placed extends { bounds: Bounds }>(
  keys: readonly Placed[],
  from: Placed,
  direction: Direction,
): Placed | null {
  const start = centre(from.bounds);
  let nearest: Placed | null = null;
  let nearestScore = Number.POSITIVE_INFINITY;
  for (const candidate of keys) {
    const end = centre(candidate.bounds);
    const horizontal = direction === "left" || direction === "right";
    const sign = direction === "right" || direction === "down" ? 1 : -1;
    const along = sign * (horizontal ? end.x - start.x : end.y - start.y);
    const across = Math.abs(horizontal ? end.y - start.y : end.x - start.x);
    // Skips the start key and every key level with it or behind it.
    if (along <= 0.01) {
      continue;
    }
    const score = along + 2 * across;
    if (score < nearestScore) {
      nearest = candidate;
      nearestScore = score;
    }
  }
  return nearest;
}
