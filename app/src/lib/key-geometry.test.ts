import { describe, expect, it } from "vitest";
import type { Bounds, Rotation } from "./bindings";
import { keyExtent } from "./key-geometry";

const UNIT_KEY: Bounds = { x: 1, y: 0, width: 1, height: 1 };

function placed(rotation: Rotation | null, secondaryBounds: Bounds | null = null) {
  return { bounds: UNIT_KEY, secondaryBounds, rotation };
}

describe("keyExtent", () => {
  it("is the rectangle's edges for a key that is not rotated", () => {
    expect(keyExtent(placed(null))).toEqual({ left: 1, top: 0, right: 2, bottom: 1 });
  });

  it("includes the second rectangle of a key that is not rotated", () => {
    const secondary = { x: 0.5, y: 0, width: 1.5, height: 2 };
    expect(keyExtent(placed(null, secondary))).toEqual({ left: 0.5, top: 0, right: 2, bottom: 2 });
  });

  it("turns a key a quarter clockwise about its origin", () => {
    const extent = keyExtent(placed({ angleDegrees: 90, originX: 1, originY: 0 }));
    expect(extent.left).toBeCloseTo(0);
    expect(extent.top).toBeCloseTo(0);
    expect(extent.right).toBeCloseTo(1);
    expect(extent.bottom).toBeCloseTo(1);
  });

  it("reaches further down and across for a key turned 45 degrees", () => {
    const extent = keyExtent(placed({ angleDegrees: 45, originX: 1, originY: 0 }));
    expect(extent.right).toBeCloseTo(1 + Math.SQRT1_2);
    expect(extent.bottom).toBeCloseTo(2 * Math.SQRT1_2);
    expect(extent.left).toBeCloseTo(1 - Math.SQRT1_2);
  });

  it("turns both rectangles of an ISO Enter together", () => {
    const enter = {
      bounds: { x: 13.75, y: 2.25, width: 1.25, height: 2 },
      secondaryBounds: { x: 13.5, y: 2.25, width: 1.5, height: 1 },
      rotation: { angleDegrees: 90, originX: 13.5, originY: 2.25 },
    };
    const extent = keyExtent(enter);
    // Turned a quarter about its top-left corner, the 2u-tall bar lies to the left of the origin.
    expect(extent.left).toBeCloseTo(13.5 - 2);
    expect(extent.right).toBeCloseTo(13.5);
    expect(extent.top).toBeCloseTo(2.25);
    expect(extent.bottom).toBeCloseTo(2.25 + 1.5);
  });
});
