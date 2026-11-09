import type { Bounds, Rotation } from "./bindings";

export type Extent = { left: number; top: number; right: number; bottom: number };

type Placed = { bounds: Bounds; secondaryBounds: Bounds | null; rotation: Rotation | null };

function corners(rectangle: Bounds): [number, number][] {
  const { x, y, width, height } = rectangle;
  return [
    [x, y],
    [x + width, y],
    [x, y + height],
    [x + width, y + height],
  ];
}

/** The smallest box around the key's rectangles after its rotation, in key units. */
export function keyExtent(key: Placed): Extent {
  const { sin, cos, originX, originY } =
    key.rotation === null
      ? { sin: 0, cos: 1, originX: 0, originY: 0 }
      : {
          sin: Math.sin((key.rotation.angleDegrees * Math.PI) / 180),
          cos: Math.cos((key.rotation.angleDegrees * Math.PI) / 180),
          originX: key.rotation.originX,
          originY: key.rotation.originY,
        };
  const rectangles =
    key.secondaryBounds === null ? [key.bounds] : [key.bounds, key.secondaryBounds];
  const turned = rectangles.flatMap(corners).map(([x, y]): [number, number] => {
    const offsetX = x - originX;
    const offsetY = y - originY;
    return [originX + offsetX * cos - offsetY * sin, originY + offsetX * sin + offsetY * cos];
  });
  const xs = turned.map(([x]) => x);
  const ys = turned.map(([, y]) => y);
  return {
    left: Math.min(...xs),
    top: Math.min(...ys),
    right: Math.max(...xs),
    bottom: Math.max(...ys),
  };
}
