# openkeychron: agent guide

openkeychron is a native, local-first alternative to Keychron Launcher: remap keys, edit
macros, and change lighting and Keychron settings without Chrome, an account or telemetry.
The first target is the Keychron V5 Max (FR ISO) through the Keychron Link 2.4 GHz receiver.
It is written in Rust; the desktop app (M4) will be Tauri + SvelteKit.

This file is the contract for every agent. Read the matching rule file in `.agents/rules/`
before touching its area:

| Area | Rule file |
|---|---|
| Any Rust code or `Cargo.toml` | [`.agents/rules/rust.md`](.agents/rules/rust.md) |
| VIA or Keychron commands, fixtures | [`.agents/rules/protocol.md`](.agents/rules/protocol.md) |
| `app/` (Tauri + Svelte) | [`.agents/rules/ui.md`](.agents/rules/ui.md) |
| `Makefile`, `.github/`, lint configs | [`.agents/rules/ci.md`](.agents/rules/ci.md) |

## The plan lives on GitHub

- Every piece of work is a GitHub issue. Each milestone (M0 to M5) has an `Epic:` issue, and
  its tasks are sub-issues. Board: https://github.com/users/Androlax2/projects/2
- A task issue has **Goal**, **Done when**, **How to verify** and **Blocked by**. It is
  finished when every *Done when* item holds and its *How to verify* command passes.
- Start only issues in **Ready**. Their GitHub "blocked by" dependencies are all closed.
- Label `needs-hardware`: only Theo can do it, with the real keyboard. Never fake it.
- Work you notice outside the issue becomes a new issue or a comment, not extra changes in
  the current PR.

## Architecture

| Crate | Role |
|---|---|
| `crates/openkeychron-hid` | `HidTransport` trait, fixture format, replay and recording transports. Device discovery comes in #16 |
| `crates/openkeychron-via` | VIA protocol: versions, keymap, layers, macros (M1, M2) |
| `crates/openkeychron-keychron` | Keychron-specific commands: RGB, debounce, LKP, wireless timers (M3) |
| `crates/openkeychron-cli` | The `openkeychron` binary (M1) |
| `app/` | Tauri + SvelteKit desktop app (M4); calls the crates, holds no HID logic |

- Protocol crates reach a device only through `HidTransport`. Nothing above
  `openkeychron-hid` names `hidapi`.
- Every protocol behaviour is tested against a fixture, so it runs without the keyboard.

## Working without the keyboard

The sandbox cannot open `/dev/hidraw*`, and CI has no keyboard. So:

- Test protocol code with `ReplayTransport` and a fixture (`docs/FIXTURES.md`).
- A hand-written fixture goes in `fixtures/synthetic/` and says it is hand-written.
- Real recordings come from `needs-hardware` issues that Theo runs; they live in
  `fixtures/<model>/`.
- If an issue cannot be finished without hardware, stop and say which step needs Theo.

## Build and verify

Toolchain: Rust stable (1.99+), plus `typos` and `cargo-deny` for the full gate.

### While iterating

Run the narrowest command that can prove the change:

```sh
cargo test -p <crate> <test-filter>   # behaviour
cargo check -p <crate>                # types and API
```

Do not run the full gate after every edit.

### Before pushing

```sh
make ci   # fmt, clippy, test, doc (all with -D warnings), typos, deny
```

It is the same gate CI runs, job for job. Do not push a tree that fails it.

## Git and GitHub

- Commits follow Conventional Commits: `type(scope): imperative lowercase description`.
  Types: `feat fix refactor chore docs ci test perf build`. Scopes: crate short names
  (`hid via keychron cli app`) or `ci`, `docs`, `release`, `linux`.
- Branches: `type/kebab-description`, cut from a fresh `origin/main`.
- One issue per PR. The PR body starts with `Closes #N` and ends with the verification
  commands and their output.
- `main` has a ruleset: the six CI checks (`fmt`, `clippy`, `test`, `doc`, `typos`, `deny`)
  must pass, and `main` cannot be deleted or force-pushed. PRs are squash-merged.
- What Claude may do with git is in [`CLAUDE.md`](CLAUDE.md) under *Git authorization*.

### Keeping the board true

Move the issue's board item to **In progress** when you start, **In review** when the PR is
open, and **Done** when it merges. The board's own workflow does not move closed issues.
After a merge, move every Backlog item whose blockers are now all closed to **Ready**.

## Decisions

Non-obvious decisions are recorded with a date in `docs/DECISIONS.md` (#11). Read it before
revisiting the stack, the first target or the testing approach.
