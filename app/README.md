# cgui — desktop GUI (macOS)

Tauri + React + TypeScript GUI for [`cgui`](https://github.com/elementalcollision/cgui) /
Apple's [`container`](https://github.com/apple/container) CLI.

**macOS-only.** The host runtime (`container`) is Apple-specific, so this
GUI only ships for macOS — Windows/Linux paths were intentionally removed.

## Quickstart

```sh
# Prereqs: macOS, Node 18+, Rust 1.88+, Apple's `container` CLI on PATH
# (optional — falls back to fixture data when absent so design work doesn't
# need a live runtime)
npm install
npm run tauri dev      # native window, hot-reload both sides
```

Outside Tauri (browser-only — useful for design work):

```sh
npm run dev            # vite at http://localhost:1420; uses TS fixtures
```

## Layout

```
app/
├── src/                React + TS frontend
│   ├── theme.ts        Workbench tokens (dark/light)
│   ├── types.ts        Domain types — kept in sync with src-tauri/src/model.rs
│   ├── fixtures.ts     Browser fallback data when not in Tauri
│   ├── api.ts          invoke() wrapper + event subscribers
│   ├── components.tsx  Sidebar / TopBar / StatusBar / icons / primitives
│   ├── views.tsx       Containers / Images / Volumes / Networks / Stacks / Logs
│   ├── modals/         One file per dialog + index.ts barrel (see docs/architecture.md)
│   ├── toast.tsx       Action-failure toast queue
│   └── App.tsx         Root: state, key handling, prefs, modal routing
└── src-tauri/          Rust backend
    └── src/
        ├── runtime/    Async wrapper around the `container` CLI (mod.rs + parse, containers, images, …)
        ├── state.rs    Per-id History + CPU%/net/disk delta computation
        ├── stacks.rs   ~/.config/cgui/stacks/*.toml parser
        ├── doctor.rs   Health checks
        ├── trivy.rs    `trivy image --format json` parser
        ├── updates.rs  GitHub Releases API check
        ├── prefs.rs    UI prefs persisted to ~/.config/cgui-gui/state.json
        ├── paths.rs    Shared config-directory resolution
        ├── model.rs    Wire types (camelCase serde for round-trip with TS)
        ├── commands.rs Tauri command handlers
        └── lib.rs      Builder + 2s polling task that emits containers:tick
```

## Architecture

The full module map is in [`../docs/architecture.md`](../docs/architecture.md).

- **Initial paint** uses `invoke('list_*')` for each tab.
- **Container updates** stream over the `containers:tick` event emitted every
  2s from a backend tokio task. CPU%, network bytes/sec, and disk bytes/sec
  are computed as deltas across polls.
- **Logs and pull** stream child-process stdout line-by-line over Tauri events
  (`logs:tick`, `pull:tick`, `pull:done`).
- **Fallback to fixtures** kicks in whenever the CLI isn't available or a
  command fails, so the UI stays usable on machines without Apple's runtime
  installed.

## Smoke test

```sh
container run -d --name smoke alpine sleep 3600
cd src-tauri && cargo run --example smoke
```

The smoke binary exercises the full runtime path (ls + stats merge, deltas,
images, volumes, networks, stacks, updates) and prints results.

## Tests

```sh
cd src-tauri && cargo test
```

## Self-update

The in-app **Updates** modal uses
[`tauri-plugin-updater`](https://v2.tauri.app/plugin/updater/) to
download and install signed updates. The flow:

1. Release workflow (`release.yml`) builds a universal-binary `.app` and
   uploads `cgui_<ver>_universal.app.tar.gz` + `.sig` to the GitHub
   Release. The `.sig` is a [minisign](https://jedisct1.github.io/minisign/)
   signature produced by `tauri-action` using the private key in
   `TAURI_SIGNING_PRIVATE_KEY`.
2. Updater-manifest workflow (`updater-manifest.yml`) fires on release
   publish, fetches the assets, and writes a `latest.json` to GitHub
   Pages at `https://elementalcollision.github.io/cgui-visual/updates/latest.json`.
3. The app, launched by the user, polls that URL when they open the
   Updates modal and click **Install**. The plugin verifies the signature
   against the public key baked into `tauri.conf.json` before applying.

The updater lists a second address,
`https://elementalcollision.github.io/quayside/updates/latest.json`. It is
the bridge for the planned rename of this repository to `quayside`: a GitHub
Pages address follows the repository's name, so after the rename the first
address returns 404 and the updater moves on to the second (it tries the
addresses in order and skips one that fails). Keep both until every copy in
use is on a release that lists the second one; see
[`docs/brand/README.md`](../docs/brand/README.md).

### Setup (once)

```sh
# Generate a keypair. The .pub goes into tauri.conf.json; the private
# key contents go into the GitHub secret TAURI_SIGNING_PRIVATE_KEY.
npm run tauri signer generate -- -w cgui-updater.key

# Repo Settings → Pages → Source: GitHub Actions
# Repo Settings → Secrets → Actions:
#   TAURI_SIGNING_PRIVATE_KEY            <contents of cgui-updater.key>
#   TAURI_SIGNING_PRIVATE_KEY_PASSWORD   <if you set one>
```

The private key never enters the repo. Lose it and you can't ship
updates that existing installs will accept.

## Known gaps

- App icons are a placeholder (programmatically generated SVG); a designer
  pass would replace `src-tauri/icons/icon.source.svg` and re-run
  `npm run icons`.
- Code-signing certificates aren't provisioned. The release workflow supports
  Apple cert + notarization secrets but they're unset by default; first
  builds will be unsigned and Gatekeeper will block first launch.
- Updater public key in `tauri.conf.json` is currently a placeholder
  generated for development. Regenerate before the first real release
  and update the matching `TAURI_SIGNING_PRIVATE_KEY` secret in CI.
