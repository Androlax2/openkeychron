<script lang="ts" module>
/** Pixels per key unit: a standard 1u key is this wide in the SVG's coordinates. */
export const KEY_UNIT = 60;

/** A selected key on one layer: the layer's index and the key's matrix position. */
export type KeySelection = { layer: number; row: number; column: number };
</script>

<script lang="ts">
import type { Bounds, Key, Keymap } from "./bindings";
import { arrowDirection, neighbour } from "./keyboard-navigation";
import { fitKeycodeLabel, shortenKeycodeName } from "./keycode-label";
import { describeKeycode, hex } from "./keycode-text";

type PlacedKey = Key & { bounds: Bounds };
type MatrixPosition = { row: number; column: number };

let {
  keymap,
  onselectionchange = () => {},
}: {
  keymap: Keymap;
  /** Called when a key is selected, and again with the new layer when the layer changes. */
  onselectionchange?: (selection: KeySelection) => void;
} = $props();

// The gap between neighbouring keycaps, taken off each side of a key's rectangle.
const INSET = 3;
// Where legends start inside a key: the same distance from the key's left and right edges.
const LEGEND_MARGIN = 10;
const id = $props.id();
let layerPosition = $state(0);
let focused: MatrixPosition | null = $state(null);
let selected: MatrixPosition | null = $state(null);
let keyList: HTMLDivElement | undefined = $state();

const shownLayer = $derived(keymap.layers[layerPosition]);
const placedKeys = $derived(
  (shownLayer?.keys ?? []).filter((key): key is PlacedKey => key.bounds !== null),
);
const width = $derived(Math.max(0, ...placedKeys.map(farRight)) * KEY_UNIT);
const height = $derived(Math.max(0, ...placedKeys.map(farBottom)) * KEY_UNIT);
const tabStop = $derived(focused ?? selected ?? placedKeys[0] ?? null);
const selectedKey = $derived(placedKeys.find((key) => isAt(key, selected)) ?? null);
const selectionSummary = $derived(
  selectedKey === null
    ? "No key selected."
    : `Selected: ${baseLegend(selectedKey)} (row ${selectedKey.row}, column ${selectedKey.column}), keycode ${describeKeycode(selectedKey)}`,
);

function farRight(key: PlacedKey): number {
  const secondary = key.secondaryBounds;
  const right = key.bounds.x + key.bounds.width;
  return secondary === null ? right : Math.max(right, secondary.x + secondary.width);
}

function farBottom(key: PlacedKey): number {
  const secondary = key.secondaryBounds;
  const bottom = key.bounds.y + key.bounds.height;
  return secondary === null ? bottom : Math.max(bottom, secondary.y + secondary.height);
}

function isAt(key: MatrixPosition, position: MatrixPosition | null): boolean {
  return position !== null && key.row === position.row && key.column === position.column;
}

/** The key's rectangles in pixels, relative to its own top-left corner. */
function capRectangles(key: PlacedKey): Bounds[] {
  const rectangles =
    key.secondaryBounds === null ? [key.bounds] : [key.bounds, key.secondaryBounds];
  return rectangles.map((rectangle) => ({
    x: (rectangle.x - key.bounds.x) * KEY_UNIT + INSET,
    y: (rectangle.y - key.bounds.y) * KEY_UNIT + INSET,
    width: rectangle.width * KEY_UNIT - 2 * INSET,
    height: rectangle.height * KEY_UNIT - 2 * INSET,
  }));
}

/**
 * The keycap shows what a French AZERTY host types for the keycode, else QMK's name, else the
 * raw value of a custom or unknown keycode. QMK's name stays in the selection line.
 */
function keycodeLabel(key: Key): string {
  if (key.azertyText !== null) {
    return key.azertyText;
  }
  return key.name === null ? hex(key.keycode) : shortenKeycodeName(key.name);
}

function fittedKeycodeLabel(key: PlacedKey) {
  return fitKeycodeLabel(keycodeLabel(key), key.bounds.width * KEY_UNIT - 2 * LEGEND_MARGIN);
}

/** The base legend, or the matrix position for a layout that has no legends. */
function baseLegend(key: Key): string {
  return key.legend ?? `${key.row},${key.column}`;
}

/**
 * The base legend's lines. A long word legend on a key narrower than 1.5u, such as "Num Lock",
 * gets one line per word: at 10px it would run past the keycap's edge.
 */
function legendLines(key: PlacedKey): string[] {
  const legend = baseLegend(key);
  return legend.length > 6 && key.bounds.width < 1.5 ? legend.split(" ") : [legend];
}

function accessibleName(key: Key): string {
  return [
    baseLegend(key),
    key.shiftLegend === null ? null : `Shift ${key.shiftLegend}`,
    key.altGrLegend === null ? null : `AltGr ${key.altGrLegend}`,
    key.azertyText === null ? null : `types ${key.azertyText}`,
    `keycode ${describeKeycode(key)}`,
  ]
    .filter((part) => part !== null)
    .join(", ");
}

function select(key: PlacedKey): void {
  selected = { row: key.row, column: key.column };
  reportSelection();
}

function showLayer(position: number): void {
  layerPosition = position;
  reportSelection();
}

function reportSelection(): void {
  const layer = keymap.layers[layerPosition];
  if (selected !== null && layer !== undefined) {
    onselectionchange({ layer: layer.index, ...selected });
  }
}

function onKeyKeydown(event: KeyboardEvent, key: PlacedKey): void {
  if (event.key === "Enter" || event.key === " ") {
    event.preventDefault();
    select(key);
    return;
  }
  const direction = arrowDirection(event.key);
  if (direction === null) {
    return;
  }
  event.preventDefault();
  const next = neighbour(placedKeys, key, direction);
  if (next === null) {
    return;
  }
  keyList
    ?.querySelector<SVGGElement>(`[data-row="${next.row}"][data-column="${next.column}"]`)
    ?.focus();
}

function onTabKeydown(event: KeyboardEvent): void {
  const step = { ArrowRight: 1, ArrowLeft: -1 }[event.key];
  if (step === undefined) {
    return;
  }
  event.preventDefault();
  const count = keymap.layers.length;
  showLayer((layerPosition + step + count) % count);
  document.getElementById(`${id}-tab-${layerPosition}`)?.focus();
}
</script>

<div class="keyboard">
  <div role="tablist" aria-label="Layers">
    {#each keymap.layers as layer, position (layer.index)}
      <button
        type="button"
        role="tab"
        id="{id}-tab-{position}"
        aria-selected={position === layerPosition}
        aria-controls="{id}-panel"
        tabindex={position === layerPosition ? 0 : -1}
        onclick={() => showLayer(position)}
        onkeydown={onTabKeydown}
      >
        Layer {layer.index}
      </button>
    {/each}
  </div>

  <div role="tabpanel" id="{id}-panel" aria-labelledby="{id}-tab-{layerPosition}">
    {#if shownLayer === undefined}
      <p role="alert">The keyboard reported no layers, so there are no keys to show.</p>
    {:else}
      <!-- The listbox is a div because Biome's a11y lint refuses an interactive role on an svg.
           role="none" keeps the svg out of the accessibility tree, so its key options belong
           to the listbox. -->
      <div bind:this={keyList} role="listbox" aria-label="Keys on layer {shownLayer.index}">
        <svg role="none" viewBox="0 0 {width} {height}" {width} {height}>
          {#each placedKeys as key (`${key.row},${key.column}`)}
            {@const rectangles = capRectangles(key)}
            {@const isSelected = isAt(key, selected)}
            {@const lines = legendLines(key)}
            {@const keycodeLabelFit = fittedKeycodeLabel(key)}
            <g
              role="option"
              class="key"
              class:selected={isSelected}
              aria-selected={isSelected}
              aria-label={accessibleName(key)}
              tabindex={isAt(key, tabStop) ? 0 : -1}
              data-row={key.row}
              data-column={key.column}
              transform="translate({key.bounds.x * KEY_UNIT} {key.bounds.y * KEY_UNIT})"
              onclick={() => select(key)}
              onkeydown={(event) => onKeyKeydown(event, key)}
              onfocus={() => {
                focused = { row: key.row, column: key.column };
              }}
            >
              <title>{describeKeycode(key)}</title>
              <!-- Edges first, fills on top: two rectangles then read as one keycap outline. -->
              {#each rectangles as rectangle}
                <rect class="cap-edge" {...rectangle} rx="4" />
              {/each}
              {#each rectangles as rectangle}
                <rect class="cap-fill" {...rectangle} rx="4" />
              {/each}
              {#if key.shiftLegend !== null}
                <text class="legend" x="10" y="20">{key.shiftLegend}</text>
              {/if}
              <text
                class="legend base"
                class:word={baseLegend(key).length > 2}
                x="10"
                y={key.shiftLegend === null ? 20 : 38}
              >
                {#if lines.length === 1}
                  {lines[0]}
                {:else}
                  {#each lines as line, lineNumber}
                    <tspan x="10" dy={lineNumber === 0 ? 0 : 11}>{line}</tspan>
                  {/each}
                {/if}
              </text>
              {#if key.altGrLegend !== null}
                <text class="legend" x={key.bounds.width * KEY_UNIT - 10} y="38" text-anchor="end">
                  {key.altGrLegend}
                </text>
              {/if}
              <text
                class="keycode"
                x={LEGEND_MARGIN}
                y={key.bounds.height * KEY_UNIT - 10}
                style:font-size={keycodeLabelFit.fontSize === null
                  ? null
                  : `${keycodeLabelFit.fontSize}px`}
              >
                {keycodeLabelFit.text}
              </text>
            </g>
          {/each}
        </svg>
      </div>
    {/if}
  </div>

  <p class="selection" aria-live="polite">
    {selectionSummary}
  </p>
</div>

<style>
/* System colours only: the visual direction is not set yet. No filters or animation, for
   WebKitGTK. */
svg {
  display: block;
  max-width: 100%;
  height: auto;
}

.key {
  cursor: pointer;
}

.cap-edge {
  fill: GrayText;
  stroke: GrayText;
  stroke-width: 2;
}

.cap-fill {
  fill: ButtonFace;
}

.legend,
.keycode {
  fill: ButtonText;
  font-family: system-ui, sans-serif;
  pointer-events: none;
  user-select: none;
}

.legend {
  font-size: 14px;
}

.legend.word {
  font-size: 10px;
}

.keycode {
  font-size: 9px;
  fill: GrayText;
}

.key.selected .cap-fill {
  fill: Highlight;
}

.key.selected .legend,
.key.selected .keycode {
  fill: HighlightText;
}

/* The thick edge replaces the outline only where it is drawn, so a webview that does not
   match :focus-visible on SVG keeps its default outline. */
.key:focus-visible {
  outline: none;
}

.key:focus-visible .cap-edge {
  fill: CanvasText;
  stroke: CanvasText;
  stroke-width: 7;
}

[role="tab"][aria-selected="true"] {
  font-weight: bold;
}
</style>
