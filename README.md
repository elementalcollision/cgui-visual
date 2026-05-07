<h1 align="center">cgui-visual</h1>

<p align="center">
  <strong>Desktop GUI for Apple's <a href="https://github.com/apple/container"><code>container</code></a> runtime.</strong><br/>
  Tauri 2 · React · TypeScript · macOS only.
</p>

<p align="center">
  <a href="https://github.com/elementalcollision/cgui-visual/actions/workflows/ci.yml"><img alt="CI status" src="https://github.com/elementalcollision/cgui-visual/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://github.com/elementalcollision/cgui-visual/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/elementalcollision/cgui-visual?color=blue"></a>
  <a href="./LICENSE"><img alt="MIT license" src="https://img.shields.io/github/license/elementalcollision/cgui-visual?color=blue"></a>
  <img alt="Platform: macOS" src="https://img.shields.io/badge/platform-macOS-1d1d1f?logo=apple&logoColor=white">
  <a href="https://tauri.app"><img alt="Built with Tauri" src="https://img.shields.io/badge/built_with-Tauri_2-24C8DB?logo=tauri&logoColor=white"></a>
</p>

<p align="center">
  <img src="design_handoff_cgui/screenshots/01-containers.png" alt="cgui-visual containers tab — workbench variation, dark mode" width="820"/>
</p>

<p align="center">
  The visual companion to the <a href="https://github.com/elementalcollision/cgui"><code>cgui</code></a> ratatui TUI — same workflows, different surface.
</p>

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
