<script lang="ts">
import { onMount } from "svelte";
import type { Device, EditableKeymap } from "../lib/bindings";
import { listDevices, readKeymap } from "../lib/commands";
import Keyboard, { type KeySelection } from "../lib/Keyboard.svelte";
import RemapPanel from "../lib/RemapPanel.svelte";

let devices: Device[] | null = $state(null);
let error: string | null = $state(null);

type KeymapView =
  | { state: "reading"; device: Device }
  | { state: "read"; device: Device; editable: EditableKeymap }
  | { state: "failed"; device: Device; reason: string };

let keymapView: KeymapView | null = $state(null);
let selection: KeySelection | null = $state(null);

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
  selection = null;
  readKeymap(device.path).then(
    (editable) => {
      if (keymapView?.device.path === device.path) {
        keymapView = { state: "read", device, editable };
      }
    },
    (reason: unknown) => {
      if (keymapView?.device.path === device.path) {
        keymapView = { state: "failed", device, reason: String(reason) };
      }
    },
  );
}

/** Shows the keymap a write read back, unless the user has moved on to another device. */
function showWrittenKeymap(device: Device, editable: EditableKeymap): void {
  if (keymapView?.state === "read" && keymapView.device.path === device.path) {
    keymapView = { state: "read", device, editable };
  }
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
        {@const device = keymapView.device}
        <Keyboard
          keymap={keymapView.editable.keymap}
          onselectionchange={(selected) => {
            selection = selected;
          }}
        />
        <RemapPanel
          path={device.path}
          editable={keymapView.editable}
          {selection}
          onchange={(editable) => showWrittenKeymap(device, editable)}
        />
      {/if}
    </section>
  {/if}
</main>
