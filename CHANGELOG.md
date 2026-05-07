# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Stack up / down** — actually launches and tears down compose-style
  stacks defined in `~/.config/cgui/stacks/*.toml`. Topological order;
  full schema (env, ports, volumes, network, depends_on, args, restart,
  cap_add, cap_drop) translated to `container run -d` argv. Per-stack
  output strip on each card.
- **Stack TCP healthchecks** — frontend polls `stack_health` every 5 s
  while the Stacks tab is open; per-service health label updates from
  the live probe instead of just mirroring "running".
- **Image inspect** modal (info button on each image row).
- **Container kill** button on the DetailModal for running containers
  (with confirmation).
- **Volume reference count** — derived from container mounts; the "N
  references" line is now real.
- **Logs level filter + search** — three toggleable level pills (ERROR /
  WARN / INFO) plus a free-text search box; copy button copies only the
  visible (filtered) lines.
- **Self-update via tauri-plugin-updater** — Updates modal "Install"
  button now downloads + verifies + installs signed updates from a
  GitHub-Pages-hosted manifest, then relaunches.
- **Updater-manifest workflow** — `updater-manifest.yml` builds and
  publishes `latest.json` to GitHub Pages on every release.

### Repo

- `LICENSE` (MIT) at the repo root.
- `CONTRIBUTING.md` documenting setup, dev workflow, code conventions,
  PR + release process.
- `.github/dependabot.yml` for weekly npm/cargo updates and monthly
  GitHub Actions updates, with patch+minor grouped.

## [0.1.0] — TBD

Initial macOS-only release. Tauri + React + TypeScript GUI for Apple's
[`container`](https://github.com/apple/container) CLI / the
[`cgui`](https://github.com/elementalcollision/cgui) TUI ecosystem.

### Added

- **Containers tab** — list / inspect / start / stop / restart / delete with
  confirmation dialogs. Live CPU / memory / network / disk rates derived
  from cumulative-counter deltas across a 2 s polling tick. 24-sample
  per-container CPU history sustains the row sparkline.
- **Images tab** — list, run (dialog: name, port mappings, env, override
  command), Trivy scan (real shell-out to `trivy image --format json`),
  delete with confirmation.
- **Volumes / Networks tabs** — list with usage bars, inspect (pretty-JSON
  modal), delete with confirmation. System networks (`default`, `host`,
  `bridge`) refuse delete with a toast.
- **Stacks tab** — TOML parser for `~/.config/cgui/stacks/*.toml`. Per-service
  state derived by cross-referencing running containers by `<stack>_<svc>`
  name. Stack health rolls up.
- **Logs tab** — streaming `container logs -f <id>` over a Tauri
  `logs:tick` event. 5 000-line ring buffer. Pause / resume / clear /
  copy. Defaults to the first running container when no target picked.
- **Pull modal** — streaming `container image pull --progress=plain` over
  `pull:tick`. Progress percent extracted from each line.
- **Detail modal** — full `container inspect` JSON, command line, status /
  uptime / CPU / mem strip, footer actions (restart, exec /bin/sh in a
  spawned Terminal.app window via osascript, start/stop).
- **Doctor modal** — async port of cgui's doctor: `which`, `--version`,
  `system status`, `~/.config/cgui/profiles.toml` presence, `state.json`
  parsability, `trivy` availability, `/var/run/container.sock` presence.
- **Updates modal** — GitHub Releases API check via `curl` for
  `apple/container` and `elementalcollision/cgui`. Compares against local
  `--version` output.
- **Settings modal** — runtime profile picker (`container` / `docker` /
  `podman`) wired through `runtime::set_bin` so the next CLI call uses the
  selected binary. Resource alert thresholds. State-file footer.
- **Toast queue** — auto-dismissing error toasts for action failures
  (`withToast(label, promise)`).
- **Prefs persistence** — `dark`, `sidebarCollapsed`, `runtime`, `lastTab`
  written to `~/.config/cgui-gui/state.json` and seeded back at startup.
- **Keyboard** — `/` focuses the filter input, `Esc` closes any open modal.
- **Backend fixture fallback** — every command tries the real CLI first;
  on missing binary or non-zero exit, falls back to in-process fixtures so
  the UI stays usable on machines without Apple's runtime.
- **Browser dev mode** — outside Tauri (vite dev), `api.*` returns fixture
  data and `onLogLine` / `onPullLine` replay fixtures on a timer so the
  UI animates.
- **Tests** — 16 Rust unit tests covering parsers, deltas, format helpers,
  child-process line draining; 29 vitest tests covering theme tokens,
  toast queue, components, api fallback, modal interactions, and a
  full-`<App />` integration suite (sidebar nav, modal open/close,
  keyboard).
- **CI** — GitHub Actions workflow runs `vitest`, `vite build`,
  `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, and a
  release build on macOS for every push/PR.
- **Release pipeline** — tag-triggered macOS universal-binary build via
  `tauri-apps/tauri-action`. Apple cert / notarization secrets pass
  through env (no-op when unset).
- **App icon** — designer-pass SVG (squircle plate, gradient navy, accent
  blue C glyph with glowing terminal cursor). Source SVG committed at
  `app/src-tauri/icons/icon.source.svg`. `npm run icons` regenerates the
  full mac-only icon set.

### Notes

- This release is unsigned. First launch will trigger Gatekeeper. To ship
  signed builds, populate the Apple Developer secrets documented in
  `.github/workflows/release.yml`.
- Trivy and the cgui CLI are optional. When absent, the app shows the
  fixture set in their respective views and Doctor flags the missing
  binary. Apple's `container` CLI is similarly optional but required for
  any real data.

[Unreleased]: https://github.com/elementalcollision/cgui/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/elementalcollision/cgui/releases/tag/v0.1.0
