# cgui — GUI design handoff

**One-line:** A clickable, high-fidelity HTML prototype for a Tauri-based GUI front for `cgui` (the container manager). Three full UI variations + every screen and modal called out in the brief.

> The live HTML in `cgui.html` is the source of truth. Screenshots in this folder are static reference for embedding in docs/specs.

---

## What's in here

```
design_handoff_cgui/
├── README.md                  ← this file
├── prototype/                 ← drop-in, runs in any browser, no build step
│   ├── cgui.html
│   ├── app.jsx
│   ├── app-base.jsx           (chrome, sidebar, top bar, status bar, icons)
│   ├── app-views.jsx          (tab views — containers/images/volumes/networks/stacks/logs)
│   ├── app-modals.jsx         (detail, pull, trivy, update, doctor, settings)
│   ├── data.js                (sample MLCommons-flavored fixtures)
│   ├── theme.js               (3 variations × dark/light tokens)
│   └── tweaks-panel.jsx       (in-page tweaks UI)
└── screenshots/
    ├── 01-containers.png      ← workbench, dark, containers tab (default)
    ├── 02-images.png
    ├── 03-volumes.png
    ├── 04-networks.png
    ├── 05-stacks.png
    ├── 06-logs.png
    ├── 07-detail-modal.png    ← container inspect overlay
    ├── 08-pull-modal.png      ← image pull progress
    ├── 09-update-modal.png    ← cgui self-update dialog
    ├── 11-doctor-modal.png    ← `cgui doctor` results
    └── 12-settings-modal.png  ← runtime profile + alert thresholds
```

The Trivy scan modal is in the prototype but did not capture cleanly to disk. Open the live HTML and click **Scan** on any image row (or hit `S`) to view it.

---

## How to run the prototype

```sh
# any static server works
cd prototype
python3 -m http.server 8000
open http://localhost:8000/cgui.html
```

No build step. React 18 + Babel-standalone are loaded from unpkg. Total weight ≈ 80 KB of original code.

### Toolbar / hotkeys

| Action | Key |
|---|---|
| Filter the current list | `/` |
| Inspect highlighted row | `↵` |
| Open logs for highlighted row | `L` |
| Run Trivy scan on image | `S` |
| Close modal / cancel | `Esc` |
| Toggle dark/light | sun-icon button in top bar |

### Variations (top floating switcher)

The pill at the very top of the window swaps between the three directions. Engineering only needs to ship one.

1. **Workbench** *(default, recommended)* — Dense, devtool-conventional. CPU sparklines, group-by-stack accordion in containers list, JSON inspect pane in detail modal. Closest to OrbStack/Lazydocker conventions. Easiest to implement; least surprising to users.
2. **Editorial** — Generous whitespace, serif accents (Newsreader for resource names), magazine-style detail layout. Same data, different rhythm. Good if cgui wants a distinct identity vs the field.
3. **Terminal** — Mono everywhere, ASCII box-drawing for chrome, command-bar-first navigation, status bar reads as a real shell prompt. The TUI's visual language brought to the GUI. Strongest for the dev-power-user audience cgui already serves.

All three render the same underlying state and use the same component tree — only tokens (`theme.js`) and a handful of conditional flags differ. Picking one is purely a brand/audience call.

### Tweaks panel (toolbar toggle: "Tweaks")

- **Variation** — same as the floating switcher, kept here for parity with the in-design tweak system.
- **Dark mode** — flips theme tokens.
- **Sidebar collapsed** — icon-only sidebar (saves ~150 px on small windows).
- **Window chrome** — `mac` shows traffic-lights / native title bar (Tauri's default on macOS); `frameless` shows a custom-drawn title strip with cgui badge (closer to OrbStack, Linear, Raycast).
- **Runtime profile** — switches the displayed runtime between `container`, `docker`, `podman` (matches `~/.config/cgui/profiles.toml`).

---

## Screens — what's where

### Containers (`01-containers.png`)
- Top KPI strip: aggregate CPU / Memory / Net I/O / Disk I/O with 60s sparklines.
- Rows grouped by Compose stack (`mlperf-inference`, `bench-training`, `redis-eval`, …).
- Per-row: status pill, CPU bar, memory bar, port mappings, uptime, action cluster (logs / exec / restart / stop).
- Selection state highlights row + lights up keyboard hint footer.
- Right-rail counts (running / paused / exited) live in the section header.

### Images (`02-images.png`)
- Reference + short SHA, size, layer count, created date.
- Per-row actions: Trivy scan, run-to-container, delete.
- Sized in GiB / MiB; layer count cues whether something is bloated.

### Volumes (`03-volumes.png`)
- Driver, mount point, owner stack, size on disk.
- "Dangling" state highlighted in warning color so users can prune.

### Networks (`04-networks.png`)
- Driver, scope, subnet, attached-container badges.
- Default `bridge` / `host` are styled as system networks (muted).

### Stacks (`05-stacks.png`)
- Compose-style: stack → services → containers tree.
- Status rolls up: stack is "running" only if every service is.
- Up / Down / Restart actions at the stack level.

### Logs (`06-logs.png`)
- Log stream with monospace font, timestamp gutter, level coloring.
- Scope filter (single container / whole stack), level filter (info / warn / error), free-text search.
- Tail-on-by-default with a "Pause" affordance.

### Container detail (`07-detail-modal.png`)
- Header: image ref, status, uptime, CPU, memory.
- `command` line + full `docker inspect` JSON below it (collapsible in the live prototype).
- Footer actions: Logs, Exec /bin/sh, Stop.

### Image pull (`08-pull-modal.png`)
- Per-layer progress + total %.
- Streaming pull manifest text below.
- "Background" sends to a tray-style minimized state; `Esc` does the same.

### Self-update (`09-update-modal.png`)
- Markdown-rendered release notes (Performance / Security / Bug fixes sections).
- Toggle for "auto-install future updates."
- Open release page / Later / Install.

### Trivy scan (in prototype only)
- Severity counts as filter pills (CRITICAL / HIGH / MEDIUM / LOW).
- Findings: CVE, package, current → fixed version, "Upgrade" CTA per row.
- Free-text search by CVE or package.

### Doctor (`11-doctor-modal.png`)
- Mirrors `cgui doctor` line-by-line.
- ✓ pass / ! warn coloring, monospace.
- Footer summary: `== 6 passed · 2 warnings · 0 failures ==`.

### Settings (`12-settings-modal.png`)
- Runtime profile picker (radio cards) — first-class because cgui is multi-runtime.
- Resource alert thresholds (CPU warn / alert, Memory warn / alert).
- State-file location footer (`~/.config/cgui/state.json — auto-saved`).

---

## Design tokens

All tokens are in `prototype/theme.js` as a single `VARIATIONS` object: `workbench | editorial | terminal`, each with `dark` and `light` palettes plus shared `radius` and `density` scales.

Per-variation surfaces:
- `bg`, `surface`, `surfaceAlt` — three depth tiers.
- `fg1`, `fg2`, `fg3` — primary/secondary/tertiary text.
- `border`, `borderStrong` — divider tiers.
- `accent`, `accentSoft` — primary action color + tinted-bg variant.
- `success`, `warning`, `danger` — semantic.
- `mono`, `sans`, `serif` — font stacks.

Every component reads tokens from a single `t` object passed via props — no hard-coded colors, no per-component theme branching beyond the tokens. To add a 4th variation, copy any block in `theme.js` and add a label to the switcher in `app.jsx`.

---

## Implementation notes for the Tauri build

- **Window chrome.** macOS uses native traffic lights (Tauri's default `decorations: true`); Windows/Linux should use `decorations: false` and the frameless chrome variant — the title strip in `app-base.jsx`'s `FramelessChrome` is the spec.
- **Sidebar collapse** persists to `state.json` (key: `ui.sidebarCollapsed`).
- **Runtime profile** is sourced from `~/.config/cgui/profiles.toml`; the dropdown in Settings writes back to that file. The active profile shows on the top bar so users always know which CLI is being shelled out to.
- **Doctor** is the same checks the existing TUI runs — wire the GUI button to the same backend command, render output line-by-line (one row per check).
- **Trivy** stays optional; if `trivy` isn't on PATH, hide the scan button on image rows and surface the warning in Doctor (already shown in `11-doctor-modal.png`).
- **Logs** should stream over an event channel from the Rust side; the front-end appends to a windowed buffer (last 5k lines) to keep memory bounded.
- **Update prompt** is informational — the actual install runs the existing self-update flow.

---

## Open questions for engineering

1. **Stacks tab** assumes Compose-style grouping is detected from container labels (`com.docker.compose.project`). If cgui is meant to support non-Compose stacks (k8s, nerdctl), that grouping logic needs spec.
2. **Sparklines** in the KPI strip pull from a 60s rolling window. Confirm the runtime stats endpoint cgui already exposes can sustain that polling rate without measurable overhead.
3. **Exec /bin/sh** in the detail modal — design assumes a separate terminal window opens (cleaner than embedding xterm.js). Confirm.

---

## Open questions for design / product

1. Pick one of the three variations (or a mix — e.g. Workbench layout + Editorial type) before development starts. Carrying all three through implementation is unnecessarily expensive.
2. The "MLCommons-flavored" sample data is just for demo realism — it doesn't imply cgui is MLC-branded. The shipped GUI uses whatever images the user actually has.
3. Empty / first-run states aren't designed yet (no containers / no images / no profile). Worth a quick follow-up pass once a variation is locked.
