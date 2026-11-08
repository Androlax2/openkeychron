<script lang="ts">
import { onMount } from "svelte";
import type { Device, Keymap } from "../lib/bindings";
import { listDevices, readKeymap } from "../lib/commands";
import Keyboard from "../lib/Keyboard.svelte";

let devices: Device[] | null = $state(null);
let error: string | null = $state(null);

type KeymapView =
  | { state: "reading"; device: Device }
  | { state: "read"; device: Device; keymap: Keymap }
  | { state: "failed"; device: Device; reason: string };

let keymapView: KeymapView | null = $state(null);

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

function showKeys(device: Device): void {
  keymapView = { state: "reading", device };
  readKeymap(device.path).then(
    (keymap) => {
      if (keymapView?.device.path === device.path) {
        keymapView = { state: "read", device, keymap };
      }
    },
    (reason: unknown) => {
      if (keymapView?.device.path === device.path) {
        keymapView = { state: "failed", device, reason: String(reason) };
      }
    },
  );
}
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
          {#if device.isReceiver}
            (connect the keyboard by USB cable to see its keys)
          {:else}
            <button type="button" class="show-keys" onclick={() => showKeys(device)}>
              Show keys
            </button>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}

  {#if keymapView !== null}
    <section aria-label="Keys of {keymapView.device.name ?? keymapView.device.path}">
      {#if keymapView.state === "reading"}
        <p>Reading the keymap…</p>
      {:else if keymapView.state === "failed"}
        <p role="alert">Cannot read the keymap: {keymapView.reason}</p>
      {:else}
        <Keyboard keymap={keymapView.keymap} />
      {/if}
    </section>
  {/if}
</main>
