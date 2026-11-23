# Contributing

Thanks for considering a contribution. This project is a desktop GUI for
Apple's [`container`](https://github.com/apple/container) runtime; it ships
**macOS-only** by design.

## Setup

```sh
# Prereqs
#   - macOS
#   - Node 18+
#   - Rust 1.88+
#   - Apple's `container` CLI on PATH (optional — fixtures are used as fallback)
git clone https://github.com/elementalcollision/cgui-visual.git
cd cgui-visual/app
npm install
npm run tauri dev
```

For browser-only design work (no Tauri window, fixture data only):

```sh
npm run dev
```

## Development workflow

- **Make a change** somewhere under `app/src/` (frontend) or
  `app/src-tauri/src/` (backend).
- **Run the tests**:
  ```sh
  npm test                  # vitest, ~1s
  cd src-tauri && cargo test
  ```
- **Run the linters** before committing:
  ```sh
  cd src-tauri
  cargo fmt
  cargo clippy --all-targets -- -D warnings
  ```
- **Verify against a real container** if your change touches the runtime
  layer:
  ```sh
  container run -d --name smoke alpine sleep 3600
  cd app/src-tauri && cargo run --example smoke
  ```
  The `smoke` binary exercises `runtime::list_containers_raw`,
  `state::poll_once`, `runtime::list_images/volumes/networks`, the stacks
  parser, and the live GitHub updates check.

## Live CLI tests

`app/src-tauri/tests/live_cli.rs` drives the app's own runtime functions
against a real `container` CLI. It is opt-in: every test is `#[ignore]` and
also returns early unless `CGUI_LIVE=1`, so `cargo test` and CI never run it.

```sh
cd app/src-tauri
CGUI_LIVE=1 cargo test --test live_cli -- --ignored --nocapture
```

Needs services running and `alpine:latest` pullable; takes about a minute.
Tests only create `cgui-live-<pid>-*` resources and delete them on exit
(even on panic). Offline parser tests use real CLI output committed under
`app/src-tauri/tests/fixtures/<version>/`; refresh or add a version with
`app/src-tauri/scripts/capture-fixtures.sh <output-dir>`.

## Code conventions

- **No comments unless WHY isn't obvious** — well-named identifiers do the
  what; comments are for hidden constraints, subtle invariants, and
  workarounds.
- **Where things live.** Dialogs are one file each under `app/src/modals/`
  (add new ones to `modals/index.ts` and `ModalsHost.tsx`); CLI calls are
  grouped by area under `app/src-tauri/src/runtime/` and re-exported from
  `runtime/mod.rs`. See [`docs/architecture.md`](./docs/architecture.md).
- **Frontend types in `src/types.ts` mirror Rust serde shapes in
  `src-tauri/src/model.rs`.** Add fields to both, with serde
  `rename_all = "camelCase"` on the Rust side.
- **List/inspect commands fall back to fixtures only when the CLI isn't
  on PATH (debug builds)** — `commands.rs` calls `runtime::available()`
  and uses `fixtures::*` in that case; `api.ts` does the same outside
  Tauri. A CLI call that *fails* (e.g. services stopped) must return an
  error, never an empty list or fixture, so the UI can show it. Don't
  break the dev fallback; design work outside Tauri depends on it.
- **Streaming events use the pattern** `start_X(args)` invoke +
  `X:tick` event. See `start_log_stream` and `start_pull` for the
  template.

## Pull requests

- One concern per PR. If you find a related issue while in the change,
  flag it in the description rather than expanding scope.
- Keep diffs focused; behavior changes don't need surrounding cleanup.
- The CI workflow (`.github/workflows/ci.yml`) must pass: vitest +
  vite build + cargo fmt + cargo clippy + cargo test + cargo build
  release. Items 1 and 4 of the workflow run on every push; the rest on
  every PR too.
- If you add a user-visible change, add an entry to `CHANGELOG.md`'s
  `[Unreleased]` section.

## Releases

Releases are cut by tagging:

```sh
git tag v0.X.0 && git push --tags
```

The `release.yml` workflow then builds a universal-binary macOS bundle
and attaches it to a draft GitHub Release. See the workflow header for
signing-secret setup.

## License

By contributing, you agree your contributions are licensed under the MIT
License (see `LICENSE`).
