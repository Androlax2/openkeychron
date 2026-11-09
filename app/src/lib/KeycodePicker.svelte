<script lang="ts">
import type { KeycodeCategory } from "./bindings";
import { matchingCategories } from "./keycode-search";

let {
  categories,
  label = "Search keycodes",
  layerCount,
  chosen,
  onchoose,
}: {
  categories: readonly KeycodeCategory[];
  /** The search field's label. */
  label?: string;
  /** The keyboard's layer count: layer keycodes are offered for these layers only. */
  layerCount: number;
  chosen: number | null;
  onchoose: (keycode: number) => void;
} = $props();

const id = $props.id();
let search = $state("");

const shownCategories = $derived(matchingCategories(categories, search, layerCount));
const matchCount = $derived(
  shownCategories.reduce((count, category) => count + category.keycodes.length, 0),
);
</script>

<div class="keycode-picker">
  <label for="{id}-search">{label}</label>
  <input id="{id}-search" type="search" autocomplete="off" spellcheck="false" bind:value={search}>

  <p class="match-count" aria-live="polite">
    {matchCount === 0
      ? `No keycode matches “${search}”.`
      : `${matchCount} ${matchCount === 1 ? "keycode matches" : "keycodes match"}.`}
  </p>

  {#if matchCount > 0}
    <div class="categories">
      {#each shownCategories as category (category.name)}
        <fieldset>
          <legend>{category.name}</legend>
          {#each category.keycodes as choice (choice.keycode)}
            <label class="choice">
              <input
                type="radio"
                name="{id}-keycode"
                value={choice.keycode}
                checked={choice.keycode === chosen}
                onchange={() => onchoose(choice.keycode)}
              >
              {choice.name}
            </label>
          {/each}
        </fieldset>
      {/each}
    </div>
  {/if}
</div>

<style>
/* System colours only: the visual direction is not set yet. */
.categories {
  max-height: 20rem;
  overflow-y: auto;
}

.choice {
  display: inline-block;
  margin: 0 0.75rem 0.25rem 0;
  white-space: nowrap;
}
</style>
