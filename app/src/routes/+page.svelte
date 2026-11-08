<script lang="ts">
import { onMount } from "svelte";
import type { Device } from "../lib/bindings";
import { listDevices } from "../lib/commands";

let devices: Device[] | null = $state(null);
let error: string | null = $state(null);

// On mount, not at load: the page is prerendered at build time, where no Tauri backend runs.
onMount(() => {
  listDevices().then(
    (found) => {
      devices = found;
    },
    (reason: unknown) => {
      error = String(reason);
    },
  );
});
</script>

<main>
  <h1>openkeychron</h1>
  <p>The desktop app is being set up.</p>
  {#if error !== null}
    <p role="alert">Cannot list devices: {error}</p>
  {:else if devices === null}
    <p>Looking for devices…</p>
  {:else if devices.length === 0}
    <p>No Keychron device with a VIA interface found.</p>
  {:else}
    <ul>
      {#each devices as device (device.path)}
        <li>
          {device.name ?? "(no product name)"}{device.isReceiver ? ", receiver" : ""}
          at {device.path}
        </li>
      {/each}
    </ul>
  {/if}
</main>
