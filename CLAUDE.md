# CLAUDE.md

## Pre-commit checklist

Always run these before creating a commit (matches CI in `.github/workflows/`):

1. `cargo fmt -- --check` — fix any issues with `cargo fmt`
2. `cargo build --verbose` — must succeed
3. `cargo clippy -- -D warnings` — must have zero warnings
