<script lang="ts">
import type {
  EditableKeymap,
  Encoder,
  EncoderKeycode,
  Key,
  TurnDirection,
  WrapperBuilder,
} from "./bindings";
import { KEYCODE_CATEGORIES } from "./bindings";
import {
  readKeymap,
  redoKeymapChange,
  restoreKeymapBackup,
  setEncoderKeycode,
  setKeycode,
  undoKeymapChange,
} from "./commands";
import type { KeySelection } from "./Keyboard.svelte";
import KeycodeBuilder from "./KeycodeBuilder.svelte";
import KeycodePicker from "./KeycodePicker.svelte";
import { keycodeName } from "./keycode-build";
import { describeKeycode, parseRawKeycode } from "./keycode-text";

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
/** What the panel remaps on the selected key: its press, or a turn of the encoder it presses. */
type Target = "press" | TurnDirection;

const TURNS: { direction: TurnDirection; label: string; text: string }[] = [
  { direction: "counterClockwise", label: "Turn counter-clockwise", text: "counter-clockwise" },
  { direction: "clockwise", label: "Turn clockwise", text: "clockwise" },
];
type PickerMode = "list" | WrapperBuilder | "raw";

const PICKER_MODES: { mode: PickerMode; label: string }[] = [
  { mode: "list", label: "Keycode" },
  { mode: "modified", label: "Modifiers + key" },
  { mode: "modTap", label: "Mod-tap" },
  { mode: "layerTap", label: "Layer-tap" },
  { mode: "layerMod", label: "Layer + modifiers" },
  { mode: "raw", label: "Raw value" },
];

// Only a raw value reaches these. Pressing a key mapped to one leaves the keyboard unusable
// until it restarts or is set up again, so the panel asks first (protocol.md).
const RESETTING_KEYCODE_NAMES = new Set(["QK_BOOT", "QK_RBT", "EE_CLR"]);

const id = $props.id();
let mode: PickerMode = $state("list");
let chosenTarget: Target = $state("press");
let chosen: number | null = $state(null);
let rawText = $state("");
let isResetConfirmed = $state(false);
/** What the write in progress is doing, such as "Applying…"; null when none is. */
let pending: string | null = $state(null);
let outcome: Outcome | null = $state(null);

const selectedKey = $derived(selection === null ? null : keyAt(editable, selection));
const selectedEncoder = $derived(
  selection === null ? null : encoderPressedBy(editable, selection, selectedKey),
);
const target: Target = $derived(selectedEncoder === null ? "press" : chosenTarget);
const resetWarning = $derived(
  `${target === "press" ? "Pressing this key" : "Turning the knob this way"} restarts the ` +
    "keyboard, enters its bootloader or erases its settings, and the keyboard stops typing " +
    "until it starts again.",
);
const selectedPosition = $derived(
  selection === null ? null : `${selection.row},${selection.column}`,
);
const current = $derived(
  selectedKey === null ? null : currentKeycode(selectedKey, selectedEncoder, target),
);
const selectedKeyText = $derived(
  selection === null || selectedKey === null || current === null
    ? "No key selected."
    : `Layer ${selection.layer}, ${describeTarget(selectedKey, target)}: ${describeKeycode(current)}`,
);
const chosenName = $derived(
  chosen === null
    ? null
    : (keycodeName(chosen) ??
        editable.keymap.customKeycodeNames.find((entry) => entry.keycode === chosen)?.name ??
        null),
);
const chosenText = $derived(
  chosen === null ? "none chosen" : describeKeycode({ keycode: chosen, name: chosenName }),
);
const isChosenResetting = $derived(chosenName !== null && RESETTING_KEYCODE_NAMES.has(chosenName));
const isRawUnreadable = $derived(rawText.trim() !== "" && parseRawKeycode(rawText) === null);
const applyHint = $derived(
  selectedKey === null
    ? "Select a key on the keyboard first."
    : chosen === null
      ? "Choose a keycode for it."
      : chosen === current?.keycode
        ? target === "press"
          ? "The key already has this keycode."
          : "The turn already sends this keycode."
        : isChosenResetting && !isResetConfirmed
          ? target === "press"
            ? "Confirm that this key may reset the keyboard."
            : "Confirm that this turn may reset the keyboard."
          : null,
);

// Each key starts at its press: a turn chosen on the knob is not carried to the next key, nor back
// to the knob after another key. Showing the knob on another layer keeps it.
$effect.pre(() => {
  if (selectedPosition !== undefined) {
    chosenTarget = "press";
  }
});

function chooseMode(next: PickerMode): void {
  mode = next;
  chosen = null;
  rawText = "";
  isResetConfirmed = false;
}

function keyAt(keymap: EditableKeymap, at: KeySelection): Key | null {
  return (
    keymap.keymap.layers
      .find((layer) => layer.index === at.layer)
      ?.keys.find((key) => key.row === at.row && key.column === at.column) ?? null
  );
}

/** The encoder on the selected layer whose press `key` is, if it is one. */
function encoderPressedBy(
  keymap: EditableKeymap,
  at: KeySelection,
  key: Key | null,
): Encoder | null {
  return key === null || key.encoder === null
    ? null
    : (keymap.keymap.layers
        .find((layer) => layer.index === at.layer)
        ?.encoders.find((encoder) => encoder.index === key.encoder) ?? null);
}

function currentKeycode(key: Key, encoder: Encoder | null, at: Target): EncoderKeycode | Key {
  return encoder === null || at === "press" ? key : encoder[at];
}

function describeKey(key: Key): string {
  const legend = key.legend ?? `${key.row},${key.column}`;
  return `${legend} (row ${key.row}, column ${key.column})`;
}

function describeTarget(key: Key, at: Target): string {
  const turn = TURNS.find((candidate) => candidate.direction === at);
  return turn === undefined ? describeKey(key) : `${describeKey(key)} turned ${turn.text}`;
}

/** What the selected key's target sends in the keymap the keyboard read back. */
function readBack(updated: EditableKeymap, at: KeySelection | null, of: Target): string {
  const key = at === null ? null : keyAt(updated, at);
  if (at === null || key === null) {
    return "";
  }
  const sent = currentKeycode(key, encoderPressedBy(updated, at, key), of);
  return ` Read back: layer ${at.layer}, ${describeTarget(key, of)} is ${describeKeycode(sent)}.`;
}

async function write(
  progress: string,
  done: string,
  failure: string,
  send: () => Promise<EditableKeymap>,
): Promise<void> {
  const at = selection;
  const of = target;
  pending = progress;
  outcome = null;
  try {
    const updated = await send();
    onchange(updated);
    outcome = {
      failed: false,
      text: `${done}, and the whole keymap read back as expected.${readBack(updated, at, of)}`,
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
  if (target === "press" || selectedEncoder === null) {
    const assignment = { ...selection, keycode: chosen };
    write("Applying…", "Applied", "Cannot apply", () => setKeycode(path, assignment));
    return;
  }
  const assignment = {
    layer: selection.layer,
    encoder: selectedEncoder.index,
    direction: target,
    keycode: chosen,
  };
  write("Applying…", "Applied", "Cannot apply", () => setEncoderKeycode(path, assignment));
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

  {#if selectedKey !== null && selectedEncoder !== null}
    <fieldset class="target">
      <legend>What to remap</legend>
      <label>
        <input
          type="radio"
          name="{id}-target"
          value="press"
          checked={chosenTarget === "press"}
          onchange={() => {
            chosenTarget = "press";
          }}
        >
        Press: {describeKeycode(selectedKey)}
      </label>
      {#each TURNS as turn (turn.direction)}
        <label>
          <input
            type="radio"
            name="{id}-target"
            value={turn.direction}
            checked={chosenTarget === turn.direction}
            onchange={() => {
              chosenTarget = turn.direction;
            }}
          >
          {turn.label}: {describeKeycode(selectedEncoder[turn.direction])}
        </label>
      {/each}
    </fieldset>
  {/if}

  <fieldset class="kind">
    <legend>Kind of keycode</legend>
    {#each PICKER_MODES as option (option.mode)}
      <label>
        <input
          type="radio"
          name="{id}-mode"
          value={option.mode}
          checked={option.mode === mode}
          onchange={() => chooseMode(option.mode)}
        >
        {option.label}
      </label>
    {/each}
  </fieldset>

  {#if mode === "list"}
    <KeycodePicker
      categories={KEYCODE_CATEGORIES}
      layerCount={editable.keymap.layerCount}
      {chosen}
      onchoose={(keycode) => {
        chosen = keycode;
      }}
    />
  {:else if mode === "raw"}
    <div class="raw">
      <label for="{id}-raw">Raw keycode, in hexadecimal</label>
      <input
        id="{id}-raw"
        type="text"
        autocomplete="off"
        spellcheck="false"
        placeholder="0x0106"
        aria-invalid={isRawUnreadable}
        aria-describedby={isRawUnreadable ? `${id}-raw-invalid` : undefined}
        bind:value={rawText}
        oninput={(event) => {
          chosen = parseRawKeycode(event.currentTarget.value);
          isResetConfirmed = false;
        }}
      >
      {#if isRawUnreadable}
        <p class="invalid" id="{id}-raw-invalid">
          Enter one to four hexadecimal digits, such as 0x0106.
        </p>
      {/if}
      {#if isChosenResetting}
        <div class="risky">
          <p>{resetWarning}</p>
          <label>
            <input type="checkbox" bind:checked={isResetConfirmed}>
            Map {target === "press" ? "the key" : "the turn"} to {chosenName} anyway
          </label>
        </div>
      {/if}
    </div>
  {:else}
    <!-- A new builder for each mode, so a mode change starts from no parts. -->
    {#key mode}
      <KeycodeBuilder
        builder={mode}
        layerCount={editable.keymap.layerCount}
        onbuild={(keycode) => {
          chosen = keycode;
        }}
      />
    {/key}
  {/if}

  <p class="chosen">New keycode: {chosenText}</p>

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
    keyboard. All are kept until the app quits or the keyboard is unplugged.
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

.kind label,
.target label {
  margin-right: 0.75rem;
  white-space: nowrap;
}
</style>
