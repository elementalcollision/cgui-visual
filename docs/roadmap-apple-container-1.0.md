# Divergence roadmap: apple/container 1.0.0 vs cgui-visual

Source basis: `apple/container` tag `1.0.0` (commit `ee848e3`), parsed from
`Sources/ContainerCommands/` (the ArgumentParser command tree) and
`docs/command-reference.md`. Mapped against cgui-visual v0.2.5.

Status legend: ✅ covered · 🟡 partial · ❌ missing · ➖ intentionally out of scope

## 1. Compatibility verification (do first)

These are not features — they are "does v0.2.5 still work against a 1.0
runtime" checks. cgui's parsers were written against the 0.x JSON output.

| Item | Status | Notes |
|---|---|---|
| `stats` streams by default in 1.0 | ✅ safe | cgui already passes `--no-stream --format json` (`runtime.rs:202`) |
| No `restart` command in 1.0 | ✅ safe | cgui implements restart as stop→start (`runtime.rs:646`) |
| `ls --all --format json` schema | ⚠️ verify | Run `examples/smoke.rs` against a 1.0 install; field renames would break `parse_ls` |
| `image ls` / `volume ls` / `network ls` JSON schemas | ⚠️ verify | Same — smoke-test against 1.0 |
| NSDate epoch handling in timestamps | ⚠️ verify | 1.0 may have moved to RFC3339 |
| `system status` output | ⚠️ verify | Doctor parses this |
| Debug-build warning on stderr | ⚠️ verify | 1.0 prints a stderr warning banner in debug CLI builds; harmless, but our `run()` treats some stderr as failure context |

**Action:** run the smoke example against a real 1.0 install and fix any
parser drift in a patch release before starting feature work.

## 2. Command-surface matrix

### Container group

| 1.0 command | cgui v0.2.5 | Gap |
|---|---|---|
| `run` | 🟡 RunImageModal: name, ports, env, command | Missing: cpus/memory, volumes/mounts/tmpfs, network select, labels, workdir/user, `--rm`, arch/platform, capabilities, rosetta, ssh-agent, read-only, init |
| `create` (create without start) | ❌ | Low value in a GUI; fold into Run dialog as "create only" checkbox |
| `start` / `stop` | ✅ | incl. bulk actions |
| `kill` (signal) | 🟡 backend `runtime::kill` exists | Not exposed in UI; add signal picker (SIGTERM/SIGKILL/custom) |
| `delete` | ✅ | |
| `prune` (remove all stopped) | ❌ | One-click win; pairs with bulk-select |
| `list` | ✅ | 2s poll + tray count |
| `logs` (`--follow`, `-n`, `--boot`) | 🟡 follow ✅ | Missing: boot-log toggle (great for debugging failed starts), tail-n on open |
| `stats` | ✅ | CPU/mem/net/disk deltas, sparklines, SQLite trends |
| `inspect` | ✅ | Rich panel (env/mounts/network/ports/health/raw) |
| `exec` | ✅ | Embedded pty terminal + Terminal.app |
| `copy` (container ↔ host files) | ❌ | Natural GUI feature: drag-drop / file picker in DetailModal |
| `export` (fs → tar) | ❌ | Pairs with `copy`; "Export filesystem…" action |

### Image group

| 1.0 command | cgui v0.2.5 | Gap |
|---|---|---|
| `build` (Dockerfile, BuildKit-style: build-args, secrets, multi-stage targets, output types) | ❌ | **Largest single gap.** GUI build experience: pick Dockerfile/context, stream build log, tag result |
| `builder start/stop/status/delete` | ❌ | Required companion to build; surface builder VM state in Settings or Doctor |
| `pull` | ✅ | streaming progress modal |
| `push` | ❌ | Needs registry login first; pair them |
| `tag` | ❌ | Trivial backend; "Tag…" row action on Images |
| `save` / `load` (OCI tar) | ❌ | "Export image…" / "Import image…" with file dialogs |
| `prune` | ❌ | One-click win on Images view |
| `list` / `delete` / `inspect` | ✅ | incl. layers tab, Trivy scan + vuln history (cgui exceeds CLI here) |
| `registry login/logout/list` | ❌ | Settings section: registry credentials list + login form. Keychain-backed by the CLI |

### Machine group (new platform concept in 1.0)

| 1.0 command | cgui v0.2.5 | Gap |
|---|---|---|
| `machine create/run/list/inspect/logs/set/set-default/stop/delete` | ❌ | Entire concept absent. 1.0 introduces named "container machines" (VMs with cpus/memory/home-dir mount) that containers run inside. Minimum: a Machines tab or Settings panel showing machines, default marker, start/stop, resource config |

### Volume / Network groups

| 1.0 command | cgui v0.2.5 | Gap |
|---|---|---|
| `volume create` | ❌ | "New volume" button + name/size form |
| `volume prune` | ❌ | One-click win |
| `volume list/inspect/delete` | ✅ | |
| `network create` | ❌ | "New network" button |
| `network prune` | ❌ | One-click win |
| `network list/inspect/delete` | ✅ | incl. system-network delete guard |

### System group

| 1.0 command | cgui v0.2.5 | Gap |
|---|---|---|
| `system status` | ✅ | Doctor check |
| `system start/stop` (services) | ❌ | Doctor knows when services are down but offers no "Start services" fix button — natural `DoctorFix` extension |
| `system df` (disk usage) | ❌ | Dashboard-grade data: per-category disk usage bars (images/containers/volumes) |
| `system logs` | ❌ | Surface in Doctor as "View service logs" for support cases |
| `system dns create/delete/list` (local DNS domains, admin) | ❌ | Niche; list + create with sudo caveat |
| `system kernel set` | ❌ | Power-user; Settings advanced section at most |
| `system property list` | ❌ | Read-only table in Settings advanced |
| `system version` | ✅ | Doctor + Updates |

### Out of scope for the GUI (➖)

- `help`, shell plugins (`DefaultCommand` plugin dispatch), `--debug` global
  flag, progress-format flags — CLI ergonomics, not GUI surface.
- cgui features with no CLI equivalent stay ours: Stacks (TOML + compose
  import/export, topo up/down, graph, snapshots), Trivy integration, vuln
  trend history, metrics history sidecar, Doctor, component update checks.

## 3. Phased roadmap

### Phase 0 — 1.0 compatibility patch (v0.2.6)
1. Smoke-test all parsers against a 1.0 install; fix JSON drift.
2. Bump Doctor's version check to recognize 1.0 and warn on <1.0.
3. Update onboarding copy / README to reference 1.0.

### Phase 1 — cheap parity wins (v0.3.0)
1. **Prune everywhere**: container/image/volume/network prune buttons with
   confirm dialogs (4 small commands, one UI pattern).
2. **Image tag** row action.
3. **Kill with signal** picker in container row menu + DetailModal.
4. **Logs**: boot-log toggle and tail-n window on open.
5. **System df** panel (disk usage bars) — likely on a new Overview/footer.
6. **Doctor fix**: "Start services" button wired to `system start`.

### Phase 2 — registry + image lifecycle (v0.4.0)
1. Registry logins UI (list/login/logout) in Settings.
2. Image push (progress modal mirrors pull).
3. Image save/load with native file dialogs.
4. Container copy (file picker both directions) + export-filesystem action.

### Phase 3 — build experience (v0.5.0)
1. Builder lifecycle surface (status/start/stop, cpu/mem config).
2. Build modal: Dockerfile + context pickers, build-args, target stage,
   tag, no-cache; stream build output over a `build:tick` event.
3. Post-build: offer Trivy scan + Run.

### Phase 4 — machines + run-dialog depth (v0.6.0)
1. Machines management UI (list, default, start/stop, create with
   cpus/memory, inspect, logs).
2. RunImageModal expansion: resources (cpus/mem), volume/tmpfs mounts,
   network attach, labels, workdir/user, `--rm`, platform/arch, rosetta.
   Progressive disclosure — keep the simple path simple.
3. System DNS domains panel (admin-gated) if user demand exists.

## 4. Risks / notes

- **Schema drift is the only ship-blocker.** Everything in Phases 1–4 is
  additive; Phase 0 protects existing users on auto-update.
- `registry login` stores credentials in the user's keychain via the CLI —
  the GUI should shell out and never handle secrets itself.
- `machine` semantics (default machine, per-machine containers) may affect
  how `ls` scoping works; investigate during Phase 0 smoke testing.
- Apple's CLI gained a plugin mechanism (unrecognized subcommands dispatch
  to plugins). If an ecosystem emerges (e.g. compose plugins), revisit
  Stacks integration.
