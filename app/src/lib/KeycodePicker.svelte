<script lang="ts">
import { KEYCODE_CATEGORIES } from "./bindings";
import { matchingCategories } from "./keycode-search";

let {
  layerCount,
  chosen,
  onchoose,
}: {
  /** The keyboard's layer count: layer keycodes are offered for these layers only. */
  layerCount: number;
  chosen: number | null;
  onchoose: (keycode: number) => void;
} = $props();

const id = $props.id();
let search = $state("");

const shownCategories = $derived(matchingCategories(KEYCODE_CATEGORIES, search, layerCount));
</script>

<div class="keycode-picker">
  <label for="{id}-search">Search keycodes</label>
  <input id="{id}-search" type="search" autocomplete="off" spellcheck="false" bind:value={search}>

  {#if shownCategories.length === 0}
    <p class="no-match">No keycode matches “{search}”.</p>
  {:else}
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
