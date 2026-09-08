# Local development

## Prerequisites

- Node.js 24
- Rust 1.89 (`rustup component add rustfmt clippy`)
- Tauri 2 operating-system prerequisites

## Commands

```bash
make setup       # reproducible JavaScript install
npm run dev      # browser-only fixture UI
make check-web   # format, lint, typecheck, tests, production web build
make check-rust  # format, clippy, and workspace tests
npm run tauri dev
npm run tauri build
```

The browser-only route uses a fake core and never requests platform permissions. Use
`?surface=compact`, `?surface=history`, or `?surface=setup` to inspect each surface.
