# Every CI job runs one of these targets, so `make ci` locally is the same gate as CI.
# Changing a recipe here changes .github/workflows/ci.yml too.

export RUSTFLAGS := -D warnings
export RUSTDOCFLAGS := -D warnings

.PHONY: ci fmt clippy test doc typos deny

ci: fmt clippy test doc typos deny

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --workspace --all-targets --locked -- -D warnings

test:
	cargo test --workspace --locked

doc:
	cargo doc --workspace --no-deps --document-private-items --locked

typos:
	typos --config .config/typos.toml

deny:
	cargo deny --config .cargo/deny.toml check
