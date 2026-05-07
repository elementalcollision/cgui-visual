# cgui-visual

Tauri + React + TypeScript desktop GUI for Apple's
[`container`](https://github.com/apple/container) runtime. macOS-only.

The visual companion to the [`cgui`](https://github.com/elementalcollision/cgui)
ratatui TUI — same workflows, different surface.

> **Looking for the TUI?** It lives in [elementalcollision/cgui](https://github.com/elementalcollision/cgui).

## What it does

- **Containers** — list / inspect / start / stop / restart / kill / delete with live CPU, memory, network, disk rates derived from `container stats` deltas
- **Images** — list / inspect / Trivy scan / run (with name, ports, env, command form) / delete
- **Volumes / Networks** — list with reference counts, JSON inspect, delete
- **Stacks** — TOML compose-style at `~/.config/cgui/stacks/*.toml`; up/down with topological ordering; live TCP/HTTP/cmd healthchecks with `start_period_s` grace window
- **Logs** — streaming `container logs -f` with 5 000-line ring buffer, level filter, free-text search
- **Pull** — streaming `container image pull` with progress
- **Doctor** — local-environment health check
- **Self-update** — Tauri-updater plugin against a GitHub-Pages-hosted manifest

## Layout

```
.
├── app/                    Tauri + React + TS desktop app  ← start here
│   ├── README.md           Setup, dev workflow, architecture
│   ├── src/                Frontend
│   ├── src-tauri/          Rust backend
│   └── …
├── design_handoff_cgui/    Original clickable HTML prototype + screenshots
│   ├── README.md
│   ├── prototype/
│   └── screenshots/
├── CHANGELOG.md
├── CONTRIBUTING.md
└── LICENSE                 MIT
```

The bulk of the project documentation is in [`app/README.md`](./app/README.md).
The [design handoff](./design_handoff_cgui/README.md) documents the three
variations (Workbench, Editorial, Terminal) the prototype carried; the
shipping app uses the **Workbench** variation.

## Quickstart

```sh
cd app
npm install
npm run tauri dev
```

Apple's `container` CLI is optional — when absent, the app falls back to
fixture data so the UI is fully usable for design work.

## License

MIT — see [LICENSE](./LICENSE).
