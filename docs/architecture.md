# Architecture

cgui-visual is a Tauri 2 desktop app (macOS only). The React/TypeScript
frontend renders the UI; the Rust backend shells out to Apple's `container`
CLI and pushes results to the frontend over Tauri commands and events. There
is no daemon or server of its own.

```
React UI  --invoke(command)-->  commands.rs  -->  runtime/  -->  `container` CLI
   ^                                |
   +------ events (containers:tick, logs:tick, pull:tick, ...) ----+
```

Wire types live in `app/src-tauri/src/model.rs` and are mirrored by
`app/src/types.ts` (camelCase via serde). Keep the two in step.

## Frontend: `app/src/`

| Path | Role |
| --- | --- |
| `main.tsx`, `App.tsx` | Entry point and root component: app state, polling subscription, key handling, prefs, which modal is open |
| `views.tsx` | Main tabs: Containers, Images, Volumes, Networks, Stacks, Logs |
| `components.tsx` | Sidebar, top bar, status bar, icons, small primitives |
| `StackGraph.tsx`, `Terminal.tsx` | Stack dependency graph and the xterm.js terminal. Both are `React.lazy` chunks loaded from `ModalsHost` |
| `ModalsHost.tsx` | Picks the dialog for the current `Modal` value. Itself lazy-loaded from `App`, so all dialog code is one deferred chunk |
| `modals/` | One file per dialog, plus `index.ts`, a barrel that re-exports the public surface (`import { X } from './modals'`) |
| `api.ts` | `invoke()` wrapper and event subscribers. Outside Tauri it falls back to `fixtures.ts` so the UI works in a plain browser |
| `dialogs.ts`, `TextPromptHost.tsx` | Async `askConfirm` / `askText`. Confirmations use the native dialog plugin; text prompts use an in-app modal. Not lazy, because `App` needs them immediately |
| `a11y.tsx` | Shared accessible dialog shell (`DialogBackdrop`: role, labelling, Esc, focus move/trap/restore) and `srOnly` helpers |
| `toast.tsx`, `loadState.tsx` | Failure toasts (announced via `aria-live`), and shared "failed to load" / empty states so a CLI error never looks like an empty list |
| `cliVersion.ts` | Detected CLI version and `useCliAtLeast(min)`, used to gate 1.6+/1.7+ features |
| `validate.ts` | Inline form validators that mirror the backend checks in `runtime/` |
| `containerActions.ts` | Multi-step container actions shared by the detail dialog (commit to image) |
| `theme.ts`, `types.ts`, `fixtures.ts` | Design tokens, domain types, offline demo data |

### `modals/`

- `DetailModal.tsx` with `InspectPanel.tsx` (tabs, env/mounts/network/ports/health) and `TrendsPanel.tsx` (history sparklines).
- `PullModal`, `BuildModal`, `UpdateModal`, `DoctorModal`, `OnboardingModal`, `HelpModal`, `RunImageModal`, `MachineLogsModal`, `JsonInspectModal`, `ImageInspectModal`.
- `TrivyModal.tsx` with `CveDrawer.tsx` and `VulnTrendStrip.tsx`.
- `CreateVolumeModal`, `CreateNetworkModal`, sharing the form pieces in `CreateModalShell.tsx`.
- `SettingsModal.tsx` with its panels in `settings/` (`ServicesPanel`, `MachinesPanel`, `BuilderPanel`, `RegistryPanel`, `DiskUsagePanel`, `SettingsToggle`).
- `CommandPalette.tsx` (Cmd-K, includes `fuzzyScore`).
- `shared.tsx` (`Backdrop`, the dialog wrapper every modal uses) and `inspect.ts` (pure `parseInspect` / `parseImageInspect` parsers).

New dialogs go in their own file under `modals/`, are exported from
`index.ts`, and get a case in `ModalsHost.tsx`. Always give `Backdrop` a
`title`: it is the dialog's accessible name.

## Backend: `app/src-tauri/src/`

| Module | Role |
| --- | --- |
| `lib.rs`, `main.rs` | Builder, tray / menu-bar mode, global hotkey, the 2 s polling task that emits `containers:tick`, and the command registration list |
| `commands.rs` | `#[tauri::command]` handlers. List and stream commands check `runtime::available()`; debug builds fall back to `fixtures.rs` when the CLI is missing, release builds return an error |
| `runtime/` | The only code that spawns `container`. See below |
| `model.rs` | Wire types shared with the frontend |
| `state.rs` | Per-container history and CPU / network / disk rate computation across polls. Uses the CLI's native CPU figure (1.7.1+) when present |
| `history.rs` | SQLite sidecar (`~/.config/cgui-gui/history.db`, owner-only) with 7-day metric and log history and 90-day Trivy scan summaries |
| `logpersist.rs` | Persists log lines separately from the display stream, so reopening the Logs view does not duplicate history |
| `stacks.rs`, `compose.rs`, `snapshot.rs` | Stacks: TOML schema, up/down ordering and healthchecks (`stacks.rs`); `docker-compose.yml` import / export (`compose.rs`, uses `serde-saphyr`); stack config snapshot and restore (`snapshot.rs`) |
| `pty.rs` | Embedded terminal sessions: a pty running `container exec -it`, streamed over `pty:tick:<id>` events |
| `updates.rs` | Release check against the GitHub API through `/usr/bin/curl` |
| `prefs.rs` | UI preferences in `~/.config/cgui-gui/state.json` |
| `paths.rs` | `config_dir(app_dir)`: the one place `$XDG_CONFIG_HOME` / `$HOME/.config` is resolved |
| `doctor.rs`, `trivy.rs` | Environment health checks; `trivy image` runner and parser |
| `menubar_hint.rs`, `timeutil.rs` | One-time menu-bar-mode notification; small time helpers |
| `fixtures.rs` | Demo data for dev builds without the CLI |

### `runtime/`

`mod.rs` holds the process plumbing: locating the binary, `run` /
`run_with_timeout` / `run_output`, `spawn` and `drain_lines` for streaming,
timeouts, argument validation (`validate_entity_name`, `reject_flag_like`),
env-value masking, `split_command`, and the CLI version cache. Everything
else is a submodule re-exported at `runtime::` level, so callers write
`runtime::list_images`, never `runtime::images::list_images`.

| Submodule | Contents |
| --- | --- |
| `parse.rs` | JSON to model parsers (`parse_container`, `parse_image`, ...), `StatRow`, reference counts, duration and byte formatting |
| `containers.rs` | List, stats, inspect, start / stop / kill / delete / restart, run from image (`RunArgs`, `run_image_argv`), copy and export |
| `images.rs` | List (with partial-failure warning), inspect, tag, delete, commit, build, save, load |
| `volumes_networks.rs` | List, inspect, delete, create, and the name / size / subnet / label validators |
| `system.rs` | Services start / stop / restart / status, builder VM, `system df`, prune, clean |
| `machine.rs` | `container machine ...` including user options, logs and inspect |
| `registry.rs` | Registry list / login / logout |
| `terminal.rs` | Open Terminal.app on `exec -it`, with shell quoting |
| `errors.rs` | Classification of CLI error text (not found, not running, services down) |
| `features_test.rs`, `fixtures_test.rs` | Offline tests for the 1.6/1.7 features and for parsers run over real CLI output in `tests/fixtures/<version>/` |

Unit tests live next to the code they cover. Items one submodule shares with
another are `pub(super)`, visible only inside `runtime`.

## Tests

- Frontend: `npm test` (Vitest, jsdom). Dialog tests import from the `modals` barrel.
- Backend: `cargo test` from `app/src-tauri`.
- Live: `CGUI_LIVE=1 cargo test --test live_cli -- --ignored` drives the real CLI; see `CONTRIBUTING.md`.
