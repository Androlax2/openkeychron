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
| Node.js | 26.13.0 | the app's checks in `make fmt`, `make clippy` and `make test`, `tauri dev` |
| `pnpm` | 12.17.0 (`packageManager` in `app/package.json`) | the app's dependencies |

CI installs `typos`, `cargo-deny`, Node.js and `pnpm` at these exact versions, so match them
locally. On Arch:

```sh
sudo pacman -S --needed rust make typos cargo-deny nodejs pnpm webkit2gtk-4.1
```

With rustup instead of a distribution Rust, `rust-toolchain.toml` selects stable with
`rustfmt` and `clippy`.

### System packages

Device access uses the `hidapi` crate's pure-Rust Linux backend (`linux-native-basic-udev`),
which links no system library; see `docs/DECISIONS.md`. The desktop app does:

| Package | Arch | Debian/Ubuntu (what CI installs) | Why |
|---|---|---|---|
| WebKitGTK 4.1 | `webkit2gtk-4.1` | `libwebkit2gtk-4.1-dev` | Tauri's webview on Linux; `app/src-tauri` is a workspace member, so `cargo build --workspace` needs it |

New packages arrive with the issue whose code needs them, and that issue adds a row here.

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

## The desktop app (`app/`)

Tauri 2 with SvelteKit (static adapter) and TypeScript. Install its dependencies once per
clone, and again after `app/pnpm-lock.yaml` changes:

```sh
pnpm --dir app install --frozen-lockfile
pnpm --dir app check       # Biome format and lint, then svelte-check; any warning fails
pnpm --dir app test        # component tests (Vitest in jsdom)
pnpm --dir app tauri dev   # opens the window, reloading on changes
```

`make fmt`, `make clippy` and `make test` run the same app checks, so they need that install too.

## The full gate

```sh
make ci
```

It runs `fmt`, `clippy`, `test`, `doc`, `typos` and `deny`, with every warning treated as an
error. `fmt` also checks the app's formatting with Biome; `clippy` also runs Biome's linter and
`svelte-check`; `test` also runs the app's component tests. Each is also its own target
(`make clippy`, `make test`…) and its own CI job of the same name. Those six jobs are the
required checks on `main`, so a green `make ci` before pushing means a green PR.

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

replays `fixtures/synthetic/via-protocol-version.toml` through `ReplayTransport`. The CLI runs
any command against a recording the same way:

```sh
cargo run -p openkeychron-cli -- info --fixture fixtures/synthetic/cli-info.toml
```

### The app in mock mode

Set `OPENKEYCHRON_FIXTURE` to a directory of fixtures and the app talks to them instead of the
connected keyboards. Give an absolute path: a relative one is resolved from the app's working
directory, not the repository root.

For a whole UI session on one device, every panel and edits that read back, put a mock
keyboard file in the directory:

```sh
mkdir -p /tmp/openkeychron-mock
cp fixtures/synthetic/v5-max-iso.mock.toml /tmp/openkeychron-mock/
OPENKEYCHRON_FIXTURE=/tmp/openkeychron-mock pnpm --dir app tauri dev
```

- Each `.toml` file in the directory is one device, listed in file name order with the path
  `fixture:<directory>/<file>`. Other files are skipped. The app never looks for a keyboard in
  mock mode.
- A file named `*.mock.toml` is a mock keyboard: a stateful fake V5 Max ISO on a cable
  (`crates/openkeychron-mock`). It answers `read_info`, `read_keymap`, `read_macros` and
  `read_settings`, and a write (keymap, macros, debounce, Snap Click, wireless timers) changes
  its state, so the next read shows it. It also holds the lighting the clients read and write:
  VIA's RGB matrix values (brightness, effect, speed, colour), the `0xA2` feature bits, and
  Keychron's per-key colours, LED lookup, mixed RGB regions and effect lists (`0xA8`), with
  the V5 Max ISO's 98 LEDs and its key-to-LED map. It starts from a synthetic seed (four
  layers of zero keycodes, sixteen empty macros, debounce 50 ms, effect 1 with every other
  lighting value at zero, every LED in mixed region 0, empty effect lists) each time the app
  starts, and never touches the file. Effect numbers are synthetic: it takes effects 1 to 24,
  with 24 as mixed RGB. Its `0xA2` feature bits name only what it answers (dynamic debounce,
  Snap Click, Keychron RGB), not every bit a real V5 Max sets. A request it does not implement (for example the indicator or per-key
  type commands) fails with "the mock keyboard does not answer request …", and a value the
  firmware would silently change (an effect past 24, brightness while lighting is off) fails
  with "the mock keyboard refuses …".
- Any other `.toml` file is a fixture. Every command replays it from the start through
  `ReplayTransport`, like the CLI's `--fixture`. A fixture device therefore answers only the
  command it recorded, as often as the UI asks: `cli-keymap.toml` answers `read_keymap`, and
  any other command on it fails with the replay's "write #0 sent …" error. Writes are checked
  against the fixture in memory and go nowhere else: a later read still returns what the
  fixture recorded.
- The app refuses to start when the variable is empty, the directory cannot be read or holds
  no `.toml` file, or a fixture or mock keyboard file is invalid or not from a Keychron VIA
  interface. The reason is printed on stderr.

A mock keyboard file holds only the identity it is listed under:

```toml
format_version = 1

[device]
name = "mock V5 Max ISO"
vendor_id = 13364     # 0x3434
product_id = 2385     # 0x0951: the model needs a built-in layout
usage_page = 65376    # 0xff60
usage = 97            # 0x61
```

## Talking to the real keyboard (Linux)

When both a keyboard and receiver are connected, `openkeychron list` shows their paths.
Pass the chosen path with `--device <PATH>` to `info`, `settings` or `rgb`. Without it,
commands still require exactly one device. Paths can change after reconnecting, so list
again rather than reusing an old path. Selecting a receiver does not bypass write refusal.
`--device` cannot be combined with `--fixture` or used with `list`.

Not possible yet: device discovery is #16. A normal user also needs the udev rule in
`packaging/linux/`; what it opens, and how to install, check and remove it, is in
[`INSTALL-linux.md`](INSTALL-linux.md). Close Chrome, Keychron Launcher and VIA before any
session with the keyboard.

## Where things are

- [`AGENTS.md`](../AGENTS.md): crate map and the rules for agents
- [`DECISIONS.md`](DECISIONS.md): why the project is built the way it is
- [`FIXTURES.md`](FIXTURES.md): recorded device traffic
- The plan: GitHub issues and the board, https://github.com/users/Androlax2/projects/2
