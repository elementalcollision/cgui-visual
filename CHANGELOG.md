# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] — 2026-06-12

Completes Phases 1 and 2 of the apple/container 1.0 divergence
roadmap (`docs/roadmap-apple-container-1.0.md`).

### Added

- **Registry logins** in Settings — list, log in, log out
  (`container registry …`). The password is piped to the CLI over
  `--password-stdin`, never placed in argv or app state; the CLI
  stores credentials in the macOS keychain.
- **Image push** — streamed progress over `push:tick`/`push:done`
  events (same pipeline as pull). Row action on the Images tab.
- **Image save / import** — save any image as an OCI tar archive via
  a native save dialog; Import button on the Images tab loads
  archives back (`image save` / `image load`).
- **Container file operations** in the detail modal — Copy out…
  (container → folder), Copy in… (file → container), and Export tar…
  (full filesystem archive) using the CLI's `<id>:<path>` syntax.
- **Kill with signal** — the Kill button now prompts for a signal
  (default SIGKILL) and passes it via `container kill -s`.
- **Boot log toggle** in the Logs tab — streams the VM console
  (`logs --boot`) instead of stdio, for containers that die before
  their entrypoint prints anything. Console lines are excluded from
  the persisted log history.
- **Tail selector** in the Logs tab — replay only the last
  100/500/1000 lines on attach (`-n`).

## [0.3.0] — 2026-06-12

First release targeting Apple's `container` CLI **1.0.0**. See
`docs/roadmap-apple-container-1.0.md` for the full divergence roadmap
this release begins to execute.

### Fixed

- **container CLI 1.0.0 compatibility.** The 1.0 release restructured
  most list-command JSON; without this fix every container showed
  status "unknown" with no uptime, images showed "?" at 0 bytes, and
  volumes showed "?". All parsers (`ls`, `image ls`, `volume ls`,
  `network ls`) now accept both the 0.x and 1.0 shapes — verified
  against a live 1.0.0 install plus unit tests built from verbatim
  1.0 output. Mount-type objects (`{"volume": {...}}`) are handled so
  volume reference counts work again.

### Added

- **Prune** button in the top bar, scoped to the active tab —
  containers (all stopped), images (dangling), volumes (unreferenced),
  networks (unconnected). Confirms first, then toasts the CLI's
  reclaimed-space summary.
- **Tag** action on image rows (`container image tag`).
- **Disk usage** panel in Settings backed by `container system df`:
  active counts, total size, and reclaimable space per category.
- Networks table now shows the **gateway** (exposed by 1.0's
  `network ls`).

## [0.2.5] — 2026-05-15

### Fixed

- **Demo fixtures no longer leak into release builds.** When Apple's
  `container` CLI was unavailable, every list/inspect/scan/log/pull
  command unconditionally fell back to the in-process MLPerf demo
  fixtures, cluttering the experience on machines without a runtime.
  Fixture fallbacks are now gated behind `cfg!(debug_assertions)` via a
  new `dev_fixture_or<T>(fixture, empty)` helper in `commands.rs`. In
  release builds, missing-runtime paths return empty lists or surface a
  real error; only debug builds keep the fixtures for design work.
  Verified the shipped binary contains zero `mlperf-` references.

## [0.1.1] — 2026-05-07

### Added

- **macOS code-signing + notarization.** Releases now ship as Developer ID
  signed and Apple-notarized universal binaries. First-launch Gatekeeper
  warning is gone. Notarization ticket is stapled to the bundle so it
  doesn't require a network round-trip on each user's first run.
- **Auto-fire updater-manifest workflow** from `release.yml`'s
  `publish-release` job via `gh workflow run`. Sidesteps GitHub's
  anti-recursion guard which suppressed the `release: published` event
  in v0.1.0 (manual workflow_dispatch was needed). Manifest workflow
  retains the `release: published` trigger as a fallback for
  manually-published releases.

### Fixed

- `smoke` Rust binary lived under `src/bin/` and got auto-discovered by
  the Tauri bundler when building `--target universal-apple-darwin`,
  failing with "smoke does not exist" because the binary was only built
  for the host architecture. Moved to `examples/smoke.rs`; run via
  `cargo run --example smoke`. Cargo examples are not picked up by the
  bundler.

### Changed

- Updater public key rotated. Old key id `66A8237E2AC689D6` (development
  key, generated locally without a passphrase) is no longer trusted by
  the shipping app. New key id is `2234EEE99B999AAB`.

## [0.1.0] — 2026-05-07

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

[Unreleased]: https://github.com/elementalcollision/cgui-visual/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/elementalcollision/cgui-visual/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/elementalcollision/cgui-visual/compare/v0.2.5...v0.3.0
[0.2.5]: https://github.com/elementalcollision/cgui-visual/compare/v0.2.4...v0.2.5
[0.1.1]: https://github.com/elementalcollision/cgui-visual/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/elementalcollision/cgui-visual/releases/tag/v0.1.0
