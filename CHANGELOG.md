# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.4] — 2026-11-23

### Changed

- **Updates will keep arriving after the project is renamed.** cgui is
  going to be renamed Quayside, and the address cgui checks for updates
  changes with the project's name. cgui now also checks the future address,
  so this version and later ones find new releases before and after the
  rename. Copies older than this one will need to be updated by hand once
  the rename happens.

## [0.7.3] — 2026-11-23

### Fixed

- **The Logs header shows when a stream has ended.** It kept showing the
  pulsing green "follow" after the stream ended; it now shows a grey dot and
  why — "ended", "container stopped", "container removed", or "not live"
  when the stream couldn't start — even if the view was paused.
- **Container details now show Env, Mounts and Ports.** The three tabs were
  always greyed out because cgui looked for them where Docker puts them. They
  now list the container's environment variables, its bind mounts, named
  volumes and tmpfs mounts (read-only ones are marked `ro`), and its published
  ports written like the list view, e.g. `8080:80/tcp`, or
  `9000-9002:7000-7002/udp` for a range.
- **The container list shows published port ranges in full.** A range such as
  `-p 9000-9002:7000-7002` showed only its first pair, `9000:7000`; it now
  shows `9000-9002:7000-7002`.
- **Doctor no longer warns "runtime API socket not found".** That check was
  left over from Docker; Apple's `container` never creates the socket, so the
  warning always showed. The `container system status` check already covers
  whether the runtime is up.
- **Saved logs keep a quick restart's output.** `container` records a run's
  start time only to the second. If a container stopped and started again
  within one second, background log saving took the new run for the old one
  and skipped as many of its first lines as the old run had printed. cgui
  now checks each run's saved lines against what the log shows and stores a
  new run separately, from its first line.
- **Saved logs no longer lose or repeat lines after a burst.** While a
  container prints a burst of output, `container logs -f` sometimes splits a
  long line in two, and it leaves blank lines out. Background log saving
  counted saved lines, so after cgui was reopened it could skip one real
  line per split, or save a line twice after a blank line. It now picks up
  from how much text is saved, which splits and missing blank lines don't
  change. A line that was split is still saved as two rows next to each
  other.
- **Restarting a container no longer saves the new run's first lines
  twice.** After a quick restart (the Restart button stops and starts the
  container within a moment), background log saving could store the new
  run's first lines under the old run as well as under the new one. cgui now
  saves a line only once it has seen the container still in the same run,
  and keeps the old run's last lines, such as its shutdown messages. Saved
  logs can trail the live output by up to one refresh (2 s with the window
  open, 10 s when hidden).

### Known limits

- A container that is already running when you upgrade keeps being saved
  the old way, by line count, until it next starts, so a burst in that run
  can still lose or repeat lines.

### Internal

- A test shows that a burst of log lines sharing one timestamp is stored
  and read back in order, with the run's line count matching.
- The live tests' "log follower stopped" checks now only look at the test's
  own processes, so they pass while cgui is open (the app follows the test
  containers too).

## [0.7.2] — 2026-11-23

### Fixed

- **The ERROR / WARN log filter works with common log formats.** It used to
  recognise only bracketed tags like `[ERROR]`, so most logs were treated as
  INFO and the filter showed nothing. It now also understands plain `ERROR` /
  `WARN` words, `level=error` style fields, JSON `"level"` / `"severity"`
  keys, Python `ERROR:` prefixes and klog-style `E1123 …` lines, with
  coloured output stripped first. Lines are colour-coded by level again, and
  prose like "no errors found" isn't mistaken for an error.
- **A Logs view no longer leaves `container logs` running** for a container
  that was stopped or deleted while the view was open. The stream now ends
  with a note: "container removed" or "container stopped — log stream ended".
- **No stray backticks in messages.** Settings hints, Doctor results, error
  messages and notifications no longer show literal backtick characters.

## [0.7.1] — 2026-11-23

### Added

- **Container logs are saved in the background.** cgui now keeps the log
  history of every running container (up to 32 at once), whether or not a
  Logs view is open. Previously only lines that arrived while you had a
  container's logs open were saved.
  - This matters because `container` keeps only a container's **current
    run**: restarting a container wipes its earlier output, and deleting it
    removes the log entirely. cgui's history now keeps those earlier runs.
  - **No gaps or duplicates, even across restarts of cgui.** Lines written
    while cgui was closed are filled in when it next runs, as long as the
    container still exists. Containers that ran and exited while cgui was
    closed are caught up too.
  - Turn it off in **Settings → Logs → Save container logs in the
    background**. Logs are stored locally for 7 days; boot logs are never
    saved. Doctor shows how many containers are being followed.

### Known limits

- If a container restarts between two refreshes, the last lines of its
  previous run may be missed; `container` discards them on restart.
- Right after upgrading, log history saved by 0.7.0 for a container that's
  still running appears once more alongside the newly saved current run.

## [0.7.0] — 2026-11-23

New Apple `container` features, accessibility, and cgui is now Apple
`container` only. Tested with container 1.4.1 and 1.7.1 on macOS 26.7 and
macOS 27.2.

### Added

- **Commit a container to an image** (container 1.6+): **Commit to image…**
  in a container's details, for running or stopped containers.
- **DNS aliases** (container 1.6+) in the Run dialog, in stacks, and in
  compose import/export (`networks: {net: {aliases: [...]}}`). A container's
  aliases show on its Network tab.
- **New volume…** and **New network…**, with size, labels, IPv4/IPv6 subnets
  and internal-only networks.
- **Machines:** user options when creating a machine (container 1.6+), plus
  **Logs** and **Inspect** for each machine.
- **Native CPU %** from container 1.7.1's own measurement, accurate from the
  first refresh; older versions keep the previous calculation.
- Features that need a newer `container` are shown greyed out with a note
  saying which version they need, instead of failing.
- **Accessibility:** dialogs are announced as dialogs with their titles, Esc
  closes the top one, keyboard focus moves into a dialog and back out when it
  closes, every icon-only button has a label, Settings toggles are switches,
  notifications are announced, and container rows work from the keyboard.
  Selecting text by dragging out of a dialog no longer closes it.
- A one-time notification the first time Menu-bar mode hides the window,
  explaining how to bring it back (useful when your menu bar hides cgui's
  icon).

### Changed

- **cgui is Apple `container` only.** The Docker and Podman options are
  removed; they couldn't work, because cgui only understands `container`'s
  output. Existing settings still load.
- **Fonts are bundled**, so cgui no longer contacts Google Fonts; the
  security policy now only allows the app itself.
- **In the terminal, Esc goes to the shell** (vim, less, …). Close the
  terminal with its X button.
- The top bar no longer shows the "runtime: container" label (the status bar
  still does).

### Fixed

- **Log history no longer fills up with duplicates.** Opening a container's
  logs stored its whole log again each time. Now the full log is saved once
  and only new lines after that. Lines written while no Logs view is open
  aren't saved.
- **The Network tab** in a container's details showed nothing for Apple
  containers; it now shows networks, addresses and aliases.
- `cookie` 0.18.3 (RUSTSEC-2026-0594), via Tauri.

### Internal

- `modals.tsx` and `runtime.rs` (about 2,800 and 3,000 lines) are split into
  one file per dialog and one module per area, with no behaviour change; see
  `docs/architecture.md`.
- `chrono` (unmaintained) replaced with `time`; unused commands removed; one
  helper for the config folder.
- The opt-in live test suite now has 13 tests.

## [0.6.2] — 2026-11-23

Hardening and toolchain release.

### Security

- **Content Security Policy.** The app window now only runs its own
  scripts and talks only to the app itself; previously any policy was
  allowed. Fonts still load from Google Fonts.
- **Values that look like flags are rejected.** Any ID, name, image
  reference, path, signal or registry value that is empty or starts with `-`
  is refused before it reaches the `container` CLI, so something like `--all`
  can never be read as a flag. (For a path that really starts with `-`, use
  `./-name`.)
- **Private history database.** cgui's settings folder is now readable only
  by you (0700) and the history database files are 0600; existing files are
  tightened at startup. Logs are kept for 7 days and Trivy scans for 90 days
  (the newest two per image are always kept).
- **Environment values are masked** (`KEY=***`) in stack logs and in error
  messages that echo a `container run` command. The command itself is
  unchanged.
- **System tools by absolute path:** `/usr/bin/curl`, `/usr/bin/osascript`
  and `/usr/bin/which`.
- **SQLite updated** via rusqlite 0.43, fixing RUSTSEC-2026-0459 (memory
  disclosure via a crafted database and WAL file pair).
- **Safer compose import.** The YAML parser is now `serde-saphyr`, written in
  safe Rust with limits on aliases and nesting, so a small malicious file
  can't expand into billions of nodes. It's stricter than before; for
  example, duplicate keys are rejected.

### Fixed

- **Quoted commands work.** The Run dialog and compose `command:` /
  `entrypoint:` strings now follow shell quoting, so
  `sh -c "echo hi && sleep 5"` is three arguments, not six. An unterminated
  quote is reported as an error.
- **Logs no longer show every line twice** when you open a container's logs.
  The live stream already replays the whole log; saved history is now only
  shown when the live stream can't start.

### Changed

- Vitest 5.1, jsdom 31, jest-dom 8, TypeScript 7, base64 0.23.
- Dependabot keeps Tauri crates and `@tauri-apps/*` packages in one group
  and allows up to 10 open PRs.

## [0.6.1] — 2026-11-23

Security and data-safety hotfix. **Every version from 0.2.0 to 0.6.0 is
affected; please update.**

### Fixed

- **Confirmations now wait for your answer.** Since 0.2.0, no confirmation
  in cgui actually waited: the dialog plugin replaces `window.confirm()` with
  an asynchronous version, so the check always passed and the action ran
  immediately, with the native dialog appearing only afterwards. Affected:
  installing an update, deleting volumes (including their data), images,
  networks, machines and the builder, bulk container actions, stack down,
  prune, stopping the container services, pushing an image and overwriting a
  stack on import. All of these now wait for a native confirmation, and
  anything that goes wrong with the dialog counts as "no".
- **Kill, Retag, Copy in and Copy out work.** They asked for input with
  `prompt()`, which returns nothing inside the app, so they silently did
  nothing. They now use an in-app text box (Enter to confirm, Esc to cancel).
- **The update dialog** shows the new and current versions and the release
  notes, and only downloads after you choose **Install and Relaunch**.

Updating from 0.6.0 or earlier still uses the old update code, so that one
update installs without waiting for your answer. Updates after 0.6.1 ask
first.

## [0.6.0] — 2026-11-23

Targets Apple's `container` CLI 1.7.1; **1.4.1 or later is now required.**
Tested with container 1.4.1 and 1.7.1 on macOS 26.7.3 and macOS 27.2.1.

### Added

- **Menu-bar mode works.** With it on, closing the window hides cgui to the
  menu bar instead of quitting (no Dock icon while hidden); the tray's Show
  cgui, or opening cgui again from Spotlight or Finder, brings it back.
  Background polling slows from every 2 s to every 10 s while the window is
  hidden. The setting applies immediately.
- **Doctor checks the container version.** It reports an error below
  1.4.1, and warns when the CLI and its background server are different
  versions — which happens after upgrading until the services restart — with
  a one-click **Restart services** fix.
- **Opt-in live test suite** (`CGUI_LIVE=1 cargo test --test live_cli --
  --ignored`) that drives a real `container` install, plus parser fixtures
  captured from container 1.4.1 and 1.7.1. See CONTRIBUTING.md.

### Fixed

- **Errors are shown instead of empty lists.** When the container services
  are stopped, views say so and offer **Start services**; other failures
  show the CLI's message instead of an empty table. Build and the builder
  panel do the same for container 1.6+'s "Plugins are unavailable" error.
- **A failed Trivy scan is no longer shown as clean.** Scans report "not
  installed", "scan failed" (with the reason), a clean result, or findings,
  and failed scans are not recorded in vulnerability history.
- **Log views no longer leak or mix streams.** Switching containers or log
  options stops the previous `container logs -f`; each view only shows its
  own stream. Log history is written in batches, so a chatty container no
  longer slows the metrics recorder.
- **Quitting cleans up.** Terminal sessions and log streams are stopped on
  Quit, and closed terminal tabs no longer leave zombie processes.
- **Stacks:** `stack down` reports which services failed instead of always
  saying it succeeded, and a stack file with a typo is listed as invalid
  (with the parse error) instead of silently disappearing.
- **Restart** only ignores a "not running" stop failure; other failures are
  reported.
- **Images view with unreadable images:** container 1.6+ exits with an error
  when any image can't be read even though it lists the rest; cgui now shows
  the readable images with a warning banner.
- **Healthcheck grace period now applies.** `start_period_s` never took
  effect on container 1.x because the container start time wasn't parsed.
- **Update check** compares versions numerically, so a newer local build no
  longer shows as "update available".

### Removed

- Parsing paths for container 0.x output formats.

## [0.5.4] — 2026-11-23

Security and correctness hotfix. Verified live against Apple's `container`
CLI 1.4.1 on macOS 26.7.3 and macOS 27.2.1.

### Fixed

- **Containers with IDs longer than 8 characters can be managed again.**
  The app shortened every container ID to its first 8 characters and sent
  that to the CLI, which matches IDs exactly. Start, stop, delete, restart,
  kill, Reclaim space, logs, terminal, inspect, copy and export all failed
  for stack containers (`stack_service`) and unnamed (UUID) containers. The
  full ID is now sent everywhere; the short form is display-only. Stats are
  matched by exact ID, so `stack_web` no longer picks up `stack_worker`'s
  numbers. Container history is keyed by the full ID from now on; history
  recorded under the old short IDs ages out.
- **Stacks with a restart policy start again.** `stack up` passed
  `--restart`, which Apple's `container run` has never supported, so any
  service with `restart: always` or `on-failure` failed. The policy is still
  imported and saved, and `stack up` now logs a warning that it is ignored.
- **Stack and snapshot names can no longer write outside the stacks
  folder.** A compose file's `name:` or a snapshot's stack name was used
  directly as a file name, so `../` or an absolute path could write a
  `.toml` elsewhere. Names must now be 1–63 letters, digits, `_`, `.` or
  `-`, and are checked before anything is written.
- **Timed-out CLI calls are stopped.** A call that hit its timeout left the
  `container` process running; stack commands had no timeout at all. Every
  CLI call now ends its process on timeout, and stack commands use the same
  timeouts as the rest of the app.
- **Start, stop, delete and restart get 60 seconds instead of 8**, so a
  slow stop or cold VM start no longer reports "timed out" while it is
  still working.

### Security

- **Tauri 2.12.7 with updater plugin 2.14.1 and `rustls` 0.23.46**, which
  clears all six open Rust advisories. Only the `rustls` ones affected code
  in the macOS app (the updater's HTTPS client); the `zip` ones were in the
  lockfile only. Dev-tool advisories in Vite, Vitest, undici and
  source-map-js are fixed too; none of those ship in the app.

### Changed

- Tauri plugins and `@tauri-apps/*` packages updated to matching versions;
  React 19.3. CI and release builds use Node 24 (Node 20 is end of life).

## [0.5.3] — 2026-09-27

### Added

- **"Reclaim space" container action.** Wraps Apple container 1.4's
  `container clean`, which fstrims a running container's writable root
  filesystem and any writable block mounts, returning space freed inside
  its sparse ext4 disk images back to macOS. Non-destructive — no files
  or data are removed. Available on running containers from the
  container row, the detail pane, and the bulk-action bar (applied to
  the selected running containers). Older CLIs without the subcommand
  surface a clear "requires Apple container 1.4 or later" error instead
  of the raw plugin-not-found failure.

### Fixed

- **Doctor "fix failed" errors now actually show.** The detail modal's
  failure path called `toast()` without importing it, so a failed Doctor
  fix threw instead of surfacing its error.

## [0.5.2] — 2026-09-27

Verified against Apple's `container` CLI 1.4.1.

### Added

- **Clear error on invalid container/machine names.** Apple container 1.4+
  rejects entity names that don't match `^[a-zA-Z0-9][a-zA-Z0-9_.-]+$` or
  exceed 63 characters, surfacing only an unhelpful "container ID `…` is
  not a valid container ID". Names are now validated up front — before
  `container run` with a user-supplied `--name`, before `machine create`,
  and before any service in a stack is started — with a message that
  names the problem (length, leading character, disallowed character) and
  which stack/service it came from. A stack with one invalid derived name
  now fails before starting any of its containers, instead of half
  starting.

### Changed

- **Dependency and CI updates.** `tauri-action` v0 → v1 (the updater
  tarball is now named `cgui_<ver>_universal.app.tar.gz`; the updater
  manifest and tap workflows match it by suffix), `actions/checkout` v7,
  `actions/setup-node` v7, plus minor/patch bumps across the npm and
  Cargo lockfiles (vite, vitest, postcss, undici, serde_with, …).

### Fixed

- **`container system status` read via JSON, not table scraping.** 1.4.1's
  `--format json` emits a top-level `"status"` field (`"running"` /
  `"unregistered"` / `"not running"`, exit 1 when down); `parse_system_running`
  now checks that field first and falls back to the old table-token match
  for older CLIs that accept `--format json` but don't emit `status`.
  `system_running()` and the Doctor check both now pass `--format json`.
- **Network list shows real attached-container counts.** `container
  network ls` doesn't report a container count itself, so the Networks
  tab always showed `0`. It's now computed the same way volume ref-counts
  are: a parallel `container ls --all --format json` call, counting each
  network a *running* container is actually attached to
  (`status.networks[]`), so stopped containers (whose intended network
  only appears in `configuration.networks[]`) aren't double-counted.

## [0.5.1] — 2026-06-16

### Added

- **Start / stop the runtime services from the GUI.** `container system
  start` and `stop` are sudo-free, so they're now one-click actions: the
  Doctor "services not running" check runs Start directly (was
  copy-to-clipboard), and Settings gains a Services panel with a
  running/stopped status dot plus Start / Stop (Stop confirms first).

### Fixed

- **Image timestamps no longer render as `1970-01-01T00:00:00Z`.** Apple's
  `container image ls` reports a zeroed creationDate for images it can't
  date (builder / intermediate); the list and the inspect Layers tab now
  show `—` for missing/epoch dates via a shared, epoch-guarded formatter.
  The Images list also shows `—` instead of a misleading `0` in the
  Layers column, since `image ls` doesn't report a layer count.
- **Doctor no longer misreports stopped services as running.** The status
  check matched the substring "running", which also occurs in the stopped
  message "apiserver is not running", hiding the Start fix exactly when it
  was needed. It now matches the status row precisely.

## [0.5.0] — 2026-06-12

Completes the apple/container 1.0 divergence roadmap
(`docs/roadmap-apple-container-1.0.md`) — phases 3 and 4.

### Added

- **Dockerfile builds.** New Build modal on the Images tab: pick a
  build-context folder and optional Dockerfile, set tag, build-args,
  target stage, and no-cache, then watch `container build` output
  stream live with auto-follow and an edit-and-rebuild loop.
- **Builder panel** in Settings — status of the dedicated build VM
  with Start / Stop / Delete (delete drops the build cache; the next
  build recreates it).
- **Machines panel** in Settings for 1.0's container machines (named
  VMs that containers run inside): list with default badge and
  cpu/mem/disk summary, set-default, stop, delete, and a create form.
  Machines are created without booting and boot on first use.
- **Run dialog advanced options** (collapsed by default): CPUs,
  memory, network, platform, working directory, user, volume mounts,
  labels, remove-after-exit (`--rm`), and Rosetta. The simple
  name/ports/env/command path is unchanged.

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

[Unreleased]: https://github.com/elementalcollision/cgui-visual/compare/v0.7.4...HEAD
[0.7.4]: https://github.com/elementalcollision/cgui-visual/compare/v0.7.3...v0.7.4
[0.7.3]: https://github.com/elementalcollision/cgui-visual/compare/v0.7.2...v0.7.3
[0.7.2]: https://github.com/elementalcollision/cgui-visual/compare/v0.7.1...v0.7.2
[0.7.1]: https://github.com/elementalcollision/cgui-visual/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/elementalcollision/cgui-visual/compare/v0.6.2...v0.7.0
[0.6.2]: https://github.com/elementalcollision/cgui-visual/compare/v0.6.1...v0.6.2
[0.6.1]: https://github.com/elementalcollision/cgui-visual/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/elementalcollision/cgui-visual/compare/v0.5.4...v0.6.0
[0.5.4]: https://github.com/elementalcollision/cgui-visual/compare/v0.5.3...v0.5.4
[0.5.3]: https://github.com/elementalcollision/cgui-visual/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/elementalcollision/cgui-visual/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/elementalcollision/cgui-visual/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/elementalcollision/cgui-visual/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/elementalcollision/cgui-visual/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/elementalcollision/cgui-visual/compare/v0.2.5...v0.3.0
[0.2.5]: https://github.com/elementalcollision/cgui-visual/compare/v0.2.4...v0.2.5
[0.1.1]: https://github.com/elementalcollision/cgui-visual/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/elementalcollision/cgui-visual/releases/tag/v0.1.0
