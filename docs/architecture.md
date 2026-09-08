# Architecture

OpenHistory is a single background-capable Tauri 2 desktop process with an on-demand compact
surface and full history window.

```mermaid
flowchart TD
  A[macOS AX / Windows UIA] --> B[Platform adapters]
  B --> C[Canonical event channel]
  C --> D[Privacy policy]
  D --> E[Encrypted storage]
  E --> F[Deterministic segmentation]
  F --> G[Local summary]
  G --> H[Compact and full UI]
  G --> I[Loopback API and MCP]
```

The Rust crates are dependency-directed: platform adapters produce `openhistory-domain` events;
privacy runs before storage; segmentation and summaries consume allowed events; API and MCP expose
bounded, read-only derived records. The React UI communicates only through typed Tauri commands.
