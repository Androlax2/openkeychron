# UI rules (`app/`, Tauri + SvelteKit)

The app arrives in M4 (#33 onward). These rules hold from its first commit.

## Boundaries

- `app/` holds no HID or protocol logic. It calls typed Tauri commands that use the Rust
  crates; if the UI needs a new device behaviour, add it to a crate first, with a fixture
  test.
- SvelteKit runs with the static adapter: no server, no network calls, no telemetry.

## Linux first

The app renders in WebKitGTK on Linux, the weakest of the three system webviews and the
one Theo uses. Test there first.

- The keyboard is plain SVG with CSS. No heavy animation, blur or filter effects.
- Every interactive element works with the keyboard and has a visible focus state.

## Safety in the UI

- Every write shows its read-back result, and every change can be undone (see
  `protocol.md`).
- Show only controls the connected firmware supports.

## Checks

- Biome and `svelte-check` run in `make ci` and in CI; errors fail the build.
- UI work uses mock mode (#35) against fixtures, so it needs no keyboard.
