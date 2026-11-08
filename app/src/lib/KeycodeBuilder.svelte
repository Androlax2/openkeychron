<script lang="ts">
import type { WrapperBuilder } from "./bindings";
import KeycodePicker from "./KeycodePicker.svelte";
import { BASIC_CATEGORIES, buildKeycode } from "./keycode-build";

let {
  builder,
  layerCount,
  onbuild,
}: {
  builder: WrapperBuilder;
  /** The keyboard's layer count: layers are offered up to it. */
  layerCount: number;
  /** Called with the built keycode each time a part changes; null while a part is missing. */
  onbuild: (keycode: number | null) => void;
} = $props();

// LT and LM keep the layer in four bits.
const LAYERS_A_WRAPPER_HOLDS = 16;

const DESCRIPTIONS: Record<WrapperBuilder, string> = {
  modified: "Sends the key with the modifiers held, such as LCTL(C) for Ctrl+C.",
  modTap: "Acts as the modifiers while held and as the key when tapped, such as LSFT_T(A).",
  layerTap: "Switches to the layer while held and sends the key when tapped, such as LT(1, SPC).",
  layerMod: "Switches to the layer with the modifiers held, such as LM(1, MOD_LCTL).",
};

const id = $props.id();
let ctrl = $state(false);
let shift = $state(false);
let alt = $state(false);
let gui = $state(false);
let hand = $state<"left" | "right">("left");
let layer: number | null = $state(null);
let key: number | null = $state(null);

const usesModifiers = $derived(builder !== "layerTap");
const usesLayer = $derived(builder === "layerTap" || builder === "layerMod");
const usesKey = $derived(builder !== "layerMod");
const layers = $derived(
  Array.from({ length: Math.min(layerCount, LAYERS_A_WRAPPER_HOLDS) }, (_, index) => index),
);
const built = $derived(
  buildKeycode(builder, {
    modifiers: { ctrl, shift, alt, gui, rightHand: hand === "right" },
    layer,
    key,
  }),
);
const missing = $derived(
  usesModifiers && !(ctrl || shift || alt || gui)
    ? "Tick at least one modifier."
    : usesLayer && layer === null
      ? "Choose a layer."
      : usesKey && key === null
        ? "Choose the key."
        : null,
);

$effect(() => {
  onbuild(built);
});
</script>

<div class="keycode-builder">
  <p>{DESCRIPTIONS[builder]}</p>

  {#if usesModifiers}
    <fieldset>
      <legend>Modifiers</legend>
      <label><input type="checkbox" bind:checked={ctrl}> Ctrl</label>
      <label><input type="checkbox" bind:checked={shift}> Shift</label>
      <label><input type="checkbox" bind:checked={alt}> Alt</label>
      <label><input type="checkbox" bind:checked={gui}> GUI</label>
      <span class="hand">
        <label
          ><input type="radio" name="{id}-hand" value="left" bind:group={hand}>
          Left hand</label
        >
        <label
          ><input type="radio" name="{id}-hand" value="right" bind:group={hand}>
          Right hand</label
        >
      </span>
    </fieldset>
  {/if}

  {#if usesLayer}
    <label for="{id}-layer">Layer</label>
    <select id="{id}-layer" bind:value={layer}>
      <option value={null} disabled>Choose a layer</option>
      {#each layers as index (index)}
        <option value={index}>Layer {index}</option>
      {/each}
    </select>
  {/if}

  {#if usesKey}
    <KeycodePicker
      categories={BASIC_CATEGORIES}
      label={builder === "modified" ? "Search the key" : "Search the tap key"}
      {layerCount}
      chosen={key}
      onchoose={(keycode) => {
        key = keycode;
      }}
    />
  {/if}

  {#if missing !== null}
    <p class="missing">{missing}</p>
  {/if}
</div>

<style>
/* System colours only: the visual direction is not set yet. */
fieldset label {
  margin-right: 0.75rem;
  white-space: nowrap;
}

.hand {
  display: inline-block;
  margin-left: 0.75rem;
}
</style>
