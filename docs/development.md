# Development Tooling

## Pre-commit hooks (lefthook)

Uses [lefthook](https://github.com/evilmartians/lefthook) — config in `lefthook.yml`.

Checks run in parallel on every commit touching `*.rs` files:

- `cargo fmt --check`
- `cargo clippy --workspace -- -D warnings`
- `cargo check --workspace`

If lefthook is not installed: `cargo install lefthook` or `npm i -g @evilmartians/lefthook`.
