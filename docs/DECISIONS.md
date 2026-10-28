# Decision log

Dated records of decisions that are not obvious from the code. Add an entry when a
non-obvious architectural, dependency or process decision is made or revisited; never edit an
old entry to change its meaning, add a new one that supersedes it.

## 2026-10: The plan lives in GitHub Issues and a project board

Every task is a GitHub issue with Goal, Done when, How to verify and Blocked by. Each
milestone has an epic, its tasks are sub-issues, and the "blocked by" links are GitHub issue
dependencies. The board's Ready column holds the issues whose blockers are all closed.

Why: agents start each session with no memory. A plan they can query beats a plan file that
drifts from the code. One issue per PR keeps every change
traceable to the reason it exists.

## 2026-10: The Keychron V5 Max (FR ISO), through the Keychron Link receiver, comes first

It is Theo's keyboard, so it is the only one that can be tested for real. It runs Keychron's
open-source QMK firmware, and its raw HID interface (usage page `0xFF60`, usage `0x61`) is
visible through the 2.4 GHz receiver. Other models come later (#41), once the code reads layouts
from Keychron's VIA files instead of one hard-coded board.

## 2026-10: Rust for the core, Tauri for the desktop app

Same core as OpenLogi: Rust crates for HID, the VIA protocol and Keychron's commands, plus a
CLI. For the UI, three stacks were compared:

- GPUI, as OpenLogi uses: fully native, but few ready-made widgets and thin docs.
- Tauri, a Rust backend with a system webview: fast to build, and drawing a keyboard is easy
  in SVG.
- Slint or egui: native, less polished.

Tauri won. HID access stays in Rust (`hidapi`), not WebHID, so the app does not depend on a
browser engine for the part that matters. The known cost: on Linux the webview is WebKitGTK,
the weakest of the three system webviews. Mitigation: plain SVG and CSS, no heavy effects, and
testing on Linux first (`.agents/rules/ui.md`).

## 2026-10: Svelte (SvelteKit, static adapter) for the UI

Compared with React and SolidJS. Svelte ships the smallest bundle, which keeps WebKitGTK
smooth, and the main screen (an SVG keyboard with clickable keys) does not need React's
ecosystem. SolidJS is lighter still but has fewer examples. The static adapter means no server
and no network calls.

## 2026-10: Lints as strict as OpenLogi's from the first commit

Clippy `pedantic`, `unwrap`/`expect` outside tests, reason-less suppressions and unsafe code
are all flagged, and `-D warnings` turns every warning into a failure. Strictness is cheap on
an empty workspace and expensive to add later, when every new lint means hundreds of fixes.

## 2026-10: Record and replay device traffic instead of mocking protocols

The sandbox cannot open `/dev/hidraw*` and CI has no keyboard. So real HID traffic is recorded
once (by Theo, in `needs-hardware` issues) into fixture files and replayed below the protocol
code; see `docs/FIXTURES.md`. A replay fails on the first report that differs, so a test proves
the code sends exactly what the real keyboard was sent.

Fixtures are TOML (readable in review, comments allowed) with a `format_version`. The schema
has no field for a serial number and rejects unknown keys, so a recording cannot carry one by
accident.

## 2026-10: `make` as the local CI runner

Each CI job runs one `make` target, and `make ci` runs them all. `make` was already installed;
`just` would have been another tool to install for the same result.

## 2026-10: `main` is protected and PRs are squash-merged

A ruleset requires the six CI checks and blocks deleting or force-pushing `main`, with no
bypass. Squash merges give one commit per issue on `main`. Branches do not have to be up to
date with `main` before merging: with squash merges that is safe enough, and it avoids constant
branch updates.

## 2026-10: Git hooks are plain scripts calling `make`, not prek

Compared in #14: `prek` (a fast pre-commit rewrite with its own `prek.toml`), plain scripts in
the repository, and no hooks. Plain scripts won: `.githooks/pre-commit` runs `make fmt typos`
and `.githooks/pre-push` runs `make ci`. They need nothing installed and call the same targets
as CI, so there is one gate and nothing to keep in sync. prek's extra built-in checks
(whitespace, merge markers, large files) were not worth a pre-1.0 tool and a second config
format. Revisit if the hooks grow beyond calling `make`.

The hooks are opt-in per clone (`git config core.hooksPath .githooks`), because they run with
the rights of whoever commits. CI and the `main` ruleset stay the real gate.

## 2026-10: `hidapi` with its pure-Rust Linux backend

Device access uses the `hidapi` crate (MIT) with `default-features = false` and
`linux-native-basic-udev`. Compared in #16 with the default backend, which compiles hidapi's
C code and links `libudev`: the pure-Rust one adds no C code, no system library and no CI
change, at the cost of a younger backend (`basic-udev` 0.1.x). Everything above
`openkeychron-hid` sees only `HidTransport`, so switching to the C backend is a one-line
manifest change if the pure-Rust one misbehaves.

Its Linux backend reports a failed open as text only, so `openkeychron_hid::open` first opens
the node with the standard library: that is how a permission error is recognised and turned
into "install the udev rule".
