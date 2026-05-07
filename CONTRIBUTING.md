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

## Code conventions

- **No comments unless WHY isn't obvious** — well-named identifiers do the
  what; comments are for hidden constraints, subtle invariants, and
  workarounds.
- **Frontend types in `src/types.ts` mirror Rust serde shapes in
  `src-tauri/src/model.rs`.** Add fields to both, with serde
  `rename_all = "camelCase"` on the Rust side.
- **Every backend command has a fixture fallback** — `commands.rs` calls
  `runtime::available()` and falls back to `fixtures::*` when the CLI
  isn't on PATH. Don't break this; design work outside Tauri depends on
  it.
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
