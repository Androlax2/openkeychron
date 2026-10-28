# Contributing to openkeychron

Thanks for your interest. openkeychron is in early development, so the most useful
contributions right now are bug reports, research on Keychron's protocol, and code for issues
on the roadmap.

## Start from an issue

The plan lives in [GitHub issues](https://github.com/Androlax2/openkeychron/issues) and the
[roadmap board](https://github.com/users/Androlax2/projects/2).

- Pick an issue in the **Ready** column: nothing blocks it.
- For anything not on the board, open an issue first and wait for a reply before writing
  code. It saves you work on a change that does not fit.
- Issues labelled `needs-hardware` need a real Keychron keyboard; say which model you have
  when you take one.

## Development setup

Follow [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md): Rust 1.99 or newer, plus `typos` and
`cargo-deny` for the full gate. Most protocol work needs no keyboard: tests replay recorded
device traffic ([`docs/FIXTURES.md`](docs/FIXTURES.md)).

## Making a change

1. Branch from `main`: `type/short-description`, for example `feat/keymap-read`.
2. Keep the change to one issue.
3. Commit with [Conventional Commits](https://www.conventionalcommits.org):
   `type(scope): imperative lowercase description`, for example
   `feat(via): read the keymap buffer`. Types: `feat fix refactor chore docs ci test perf build`.
4. Run the full gate before pushing:

   ```sh
   make ci
   ```

   It runs the same six checks as CI (`fmt`, `clippy`, `test`, `doc`, `typos`, `deny`), and
   all six must pass before a pull request can merge.

## Pull requests

- Start the description with `Closes #N` and fill in the template.
- Include the verification commands you ran and their output.
- New behaviour comes with tests. Code that talks to the keyboard is tested against a fixture.
- Never commit a fixture containing a serial number or text you typed.

The project's coding rules are in [`AGENTS.md`](AGENTS.md) and
[`.agents/rules/`](.agents/rules/). They are written for AI agents and apply to everyone.

## Reporting a bug

Use the **Bug report** form. Include the keyboard model, the connection (cable, 2.4 GHz
receiver or Bluetooth), the firmware version and the exact command.

## License

By contributing, you agree that your contributions are dual licensed under the
[MIT](LICENSE-MIT) and [Apache-2.0](LICENSE-APACHE) licenses, as described in the
[README](README.md#license).
