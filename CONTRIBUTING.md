# Contributing to OpenHistory

OpenHistory is privacy-sensitive desktop software. Changes should keep captured content local,
minimize data before persistence, and never introduce screenshot, audio, or raw-keystroke capture.

## Local checks

1. Install Node.js 24 and Rust 1.89 with `rustfmt` and `clippy`.
2. Run `make setup`.
3. Run `make check` before opening a pull request.
4. Use synthetic fixtures in automated tests. Never commit real activity history.

Changes to observable behavior should update the active OpenSpec change before implementation.
