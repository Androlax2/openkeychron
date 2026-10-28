# CI rules

Applies to `Makefile`, `.githooks/`, `.github/`, `.config/typos.toml`, `.cargo/deny.toml`,
`.cargo/clippy.toml` and `rust-toolchain.toml`.

## One gate, two places

- Every CI job runs exactly one `make` target; `make ci` runs them all. Changing a recipe in
  the `Makefile` without the matching job in `.github/workflows/ci.yml` (or the reverse) is
  a bug.
- The jobs are `fmt`, `clippy`, `test`, `doc`, `typos` and `deny`. Their names are the
  required checks in the `main` ruleset: renaming or removing one needs Theo to update the
  ruleset in the same change.
- CI installs `typos` and `cargo-deny` at the versions Theo has locally, so a local pass
  means a CI pass. Bump both places together.

## Workflows

- Actions are pinned to a full commit SHA with the version in a comment.
- Workflows get `permissions: contents: read` unless a job needs more, and say why.
- The agent's GitHub token has no Workflows permission. A change under `.github/workflows/`
  is committed by the agent and pushed by Theo through `ask-to-run`.

## The Makefile runs on Theo's machine

`make` targets run with Theo's full rights outside the sandbox, and so do the hooks in
`.githooks/` once he turns them on. Point out every change to either in the PR description.
Targets and hooks only run checks: they never install, push or touch files outside the
repository. Hooks only call `make` targets, so they cannot drift from CI.

## Never

- Silence a check to get green: no lint added to an allow-list, no word added to typos, no
  licence or advisory added to `deny.toml` without saying why in the PR and getting Theo's
  agreement.
- Disable or skip a test to pass CI.
