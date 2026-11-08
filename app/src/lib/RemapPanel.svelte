<script lang="ts">
import type { EditableKeymap, Key } from "./bindings";
import { KEYCODE_CATEGORIES } from "./bindings";
import {
  readKeymap,
  redoKeymapChange,
  restoreKeymapBackup,
  setKeycode,
  undoKeymapChange,
} from "./commands";
import type { KeySelection } from "./Keyboard.svelte";
import KeycodePicker from "./KeycodePicker.svelte";
import { describeKeycode } from "./keycode-text";

let {
  path,
  editable,
  selection,
  onchange,
}: {
  /** The device's `Device.path`. */
  path: string;
  editable: EditableKeymap;
  selection: KeySelection | null;
  /** Called with the keymap read back after each write that succeeded. */
  onchange: (editable: EditableKeymap) => void;
} = $props();

type Outcome = { failed: boolean; text: string };

const id = $props.id();
let chosen: number | null = $state(null);
/** What the write in progress is doing, such as "Applying…"; null when none is. */
let pending: string | null = $state(null);
let outcome: Outcome | null = $state(null);

const selectedKey = $derived(selection === null ? null : keyAt(editable, selection));
const selectedKeyText = $derived(
  selection === null || selectedKey === null
    ? "No key selected."
    : `Layer ${selection.layer}, ${describeKey(selectedKey)}: ${describeKeycode(selectedKey)}`,
);
const chosenChoice = $derived(
  KEYCODE_CATEGORIES.flatMap((category) => category.keycodes).find(
    (choice) => choice.keycode === chosen,
  ) ?? null,
);
const applyHint = $derived(
  selectedKey === null
    ? "Select a key on the keyboard first."
    : chosen === null
      ? "Choose a keycode for it."
      : chosen === selectedKey.keycode
        ? "The key already has this keycode."
        : null,
);

function keyAt(keymap: EditableKeymap, at: KeySelection): Key | null {
  return (
    keymap.keymap.layers
      .find((layer) => layer.index === at.layer)
      ?.keys.find((key) => key.row === at.row && key.column === at.column) ?? null
  );
}

function describeKey(key: Key): string {
  const legend = key.legend ?? `${key.row},${key.column}`;
  return `${legend} (row ${key.row}, column ${key.column})`;
}

/** The key's keycode in the keymap the keyboard read back, when a key was selected. */
function readBack(updated: EditableKeymap, at: KeySelection | null): string {
  const key = at === null ? null : keyAt(updated, at);
  return at === null || key === null
    ? ""
    : ` Read back: layer ${at.layer}, ${describeKey(key)} is ${describeKeycode(key)}.`;
}

async function write(
  progress: string,
  done: string,
  failure: string,
  send: () => Promise<EditableKeymap>,
): Promise<void> {
  const at = selection;
  pending = progress;
  outcome = null;
  try {
    const updated = await send();
    onchange(updated);
    outcome = {
      failed: false,
      text: `${done}, and the whole keymap read back as expected.${readBack(updated, at)}`,
    };
  } catch (reason: unknown) {
    outcome = { failed: true, text: `${failure}: ${String(reason)}${await readAgain()}` };
  } finally {
    pending = null;
  }
}

/**
 * After a failed write, reads the keymap again: a write that failed after its backup has already
 * given Undo a step, and the keyboard may have changed. Returns what to add to the failure.
 */
async function readAgain(): Promise<string> {
  try {
    onchange(await readKeymap(path));
    return "";
  } catch (reason: unknown) {
    return `. Reading the keymap again also failed: ${String(reason)}`;
  }
}

function apply(): void {
  if (selection === null || chosen === null) {
    return;
  }
  const assignment = { ...selection, keycode: chosen };
  write("Applying…", "Applied", "Cannot apply", () => setKeycode(path, assignment));
}

function undo(): void {
  write("Undoing…", "Undone", "Cannot undo", () => undoKeymapChange(path));
}

function redo(): void {
  write("Redoing…", "Redone", "Cannot redo", () => redoKeymapChange(path));
}

function restoreBackup(): void {
  write("Restoring the backup…", "Backup restored", "Cannot restore the backup", () =>
    restoreKeymapBackup(path),
  );
}
</script>

<section class="remap" aria-labelledby="{id}-heading">
  <h2 id="{id}-heading">Remap a key</h2>

  <p class="selected-key">{selectedKeyText}</p>

  <KeycodePicker
    layerCount={editable.keymap.layerCount}
    {chosen}
    onchoose={(keycode) => {
      chosen = keycode;
    }}
  />

  <p class="chosen">
    New keycode: {chosenChoice === null ? "none chosen" : describeKeycode(chosenChoice)}
  </p>

  <div class="actions">
    <button
      type="button"
      disabled={applyHint !== null || pending !== null}
      aria-describedby={applyHint === null ? undefined : `${id}-apply-hint`}
      onclick={apply}
    >
      Apply
    </button>
    <button
      type="button"
      disabled={editable.undoSteps === 0 || pending !== null}
      aria-describedby="{id}-history"
      onclick={undo}
    >
      Undo{editable.undoSteps === 0 ? "" : ` (${editable.undoSteps})`}
    </button>
    <button
      type="button"
      disabled={editable.redoSteps === 0 || pending !== null}
      aria-describedby="{id}-history"
      onclick={redo}
    >
      Redo{editable.redoSteps === 0 ? "" : ` (${editable.redoSteps})`}
    </button>
    <button
      type="button"
      disabled={!editable.canRestoreBackup || pending !== null}
      aria-describedby="{id}-history"
      onclick={restoreBackup}
    >
      Restore backup
    </button>
  </div>

  {#if applyHint !== null}
    <p class="apply-hint" id="{id}-apply-hint">{applyHint}</p>
  {/if}
  <p id="{id}-history">
    Undo takes back the app's changes one at a time, and Redo puts back what Undo took back until
    the next change. Restore backup puts back the keymap from before the app's first change on this
    keyboard. All are kept until the app quits.
  </p>

  <p role="status" aria-busy={pending !== null}>
    {pending ?? (outcome !== null && !outcome.failed ? outcome.text : "")}
  </p>
  {#if outcome?.failed}
    <p role="alert">{outcome.text}</p>
  {/if}
</section>

<style>
/* System colours only: the visual direction is not set yet. */
.actions {
  display: flex;
  gap: 0.5rem;
}
</style>
