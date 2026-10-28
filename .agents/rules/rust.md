# Rust rules

Applies to any `.rs` file and any `Cargo.toml`.

## Lints

- One lint table for the whole workspace, in the root `Cargo.toml`. Every crate has
  `[lints] workspace = true`; never add per-crate lint tables.
- Clippy `pedantic` is on, and `-D warnings` makes every warning an error.
- `unwrap()` and `expect()` are allowed in tests only (`.cargo/clippy.toml`). Elsewhere,
  return an error.
- `unsafe_code` is denied. A crate that truly needs FFI opts in at the item with
  `#[expect(unsafe_code, reason = "…")]` and a `// SAFETY:` comment on every block.
- Suppressing a lint uses `#[expect(lint, reason = "…")]`, never a bare `#[allow]`, and
  never a module-wide `#![allow]`.

## Style

- Edition 2024, `rustfmt.toml` (max width 100). `cargo fmt --all` before committing.
- Errors are `thiserror` enums, one per module boundary, with messages that say what to do
  or what was expected.
- Fail loudly: no default value that hides a device answering something unexpected.
- Public items that return a value nobody should drop get `#[must_use]`.
- Comments explain *why*. If one explains *what*, rename or restructure instead.

## Tests

- Unit tests live in a `#[cfg(test)] mod tests` at the bottom of the file. Integration
  tests in `tests/` also wrap their tests in `#[cfg(test)] mod tests` (the
  `tests_outside_test_module` lint).
- One behaviour per test. The name says the behaviour:
  `rejects_a_write_with_different_bytes`, not `test_write`.
- Prefer `assert_eq!` on the exact value or error over `assert!(x.is_err())`.
- Device behaviour is tested through `HidTransport` with a fixture, not by calling private
  helpers.

## Dependencies

- Adding or upgrading a dependency needs Theo's approval in the current conversation.
  Ask with the crate, why, and its licence.
- Shared versions go in `[workspace.dependencies]`; crates use `name.workspace = true`.
- `cargo deny` must stay green: the licence allow-list in `.cargo/deny.toml` is a
  decision, not a list to extend to make a build pass.
- `Cargo.lock` is committed. Builds and CI use `--locked`; after a manifest change, update
  it with `cargo update -w` and commit it with the change.
