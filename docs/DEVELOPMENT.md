# Development

How to build, test and check openkeychron. Agents: read [`AGENTS.md`](../AGENTS.md) first;
this file is the handbook it points to.

## Toolchain

| Tool | Version | Needed for |
|---|---|---|
| Rust (`cargo`, `rustfmt`, `clippy`) | stable, 1.99 or newer | everything |
| `make` | any | `make ci`, the local copy of CI |
| `typos` | 1.51.1 | `make typos` |
| `cargo-deny` | 0.20.2 | `make deny` |

CI installs `typos` and `cargo-deny` at these exact versions, so match them locally. On Arch:

```sh
sudo pacman -S --needed rust make typos cargo-deny
```

With rustup instead of a distribution Rust, `rust-toolchain.toml` selects stable with
`rustfmt` and `clippy`.

### System packages

None yet: the workspace builds with Rust alone. They arrive with the code that needs them:

| Package | Arrives with | Why |
|---|---|---|
| `hidapi`, `libudev` (Arch: `hidapi`, `systemd-libs`) | #16, device discovery | talking to the keyboard over hidraw |
| `webkit2gtk-4.1`, Node.js, `pnpm` | #33, the Tauri app | the desktop UI |

Each of those issues updates this table with the exact package names.

## Build and test

```sh
git clone https://github.com/Androlax2/openkeychron.git
cd openkeychron
cargo build --workspace
cargo test --workspace
```

While working on one crate, run only what can prove the change:

```sh
cargo test -p openkeychron-hid replay   # one crate, filtered by test name
cargo check -p openkeychron-hid         # types only, fastest
```

## The full gate

```sh
make ci
```

It runs `fmt`, `clippy`, `test`, `doc`, `typos` and `deny`, with every warning treated as an
error. Each is also its own target (`make clippy`, `make test`…) and its own CI job of the same
name. Those six jobs are the required checks on `main`, so a green `make ci` before pushing
means a green PR.

Builds use `--locked`. After changing a `Cargo.toml`, run `cargo update -w` and commit the
updated `Cargo.lock` with it.

## Git hooks (optional)

`.githooks/` holds two hooks that call the same `make` targets as CI:

| Hook | Runs | When |
|---|---|---|
| `pre-commit` | `make fmt typos` | every commit, fast |
| `pre-push` | `make ci` | every push, the full gate |

They are off until you turn them on in your clone:

```sh
git config core.hooksPath .githooks
```

Read both scripts first: they run with your rights. Turn them off again with
`git config --unset core.hooksPath`. Why plain scripts and not a hook manager:
[`DECISIONS.md`](DECISIONS.md).

## Running against a fixture (no keyboard)

Protocol code is tested by replaying recorded HID traffic; see [`FIXTURES.md`](FIXTURES.md).

```sh
cargo test -p openkeychron-hid replay
```

replays `fixtures/synthetic/via-protocol-version.toml` through `ReplayTransport`. The CLI's
`--fixture <file>` option (#19) will run any command against a recording the same way.

## Talking to the real keyboard (Linux)

Not possible yet: device discovery is #16, and the udev rule that lets a normal user open the
keyboard is #17. Until then, close Chrome, Keychron Launcher and VIA before any session with
the keyboard: only one program can own its HID interface at a time.

## Where things are

- [`AGENTS.md`](../AGENTS.md): crate map and the rules for agents
- [`DECISIONS.md`](DECISIONS.md): why the project is built the way it is
- [`FIXTURES.md`](FIXTURES.md): recorded device traffic
- The plan: GitHub issues and the board, https://github.com/users/Androlax2/projects/2
