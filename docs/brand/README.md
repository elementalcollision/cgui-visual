# Quayside brand

cgui-visual will be renamed **Quayside** at the next major release. The
app's current icon (`app/src-tauri/icons/icon.source.svg`) stays until then.

<img src="quayside-icon-256.png" alt="Quayside icon: a ship-to-shore dock crane lifting an orange container over water, on a dark navy rounded square" width="128"/>

| File | What it is |
| --- | --- |
| `quayside-icon.svg` | Source. A ship-to-shore dock crane in the app's blue, lifting an orange shipping container, on a navy plate. |
| `quayside-icon-1024.png` | 1024 × 1024 render, the input for `tauri icon`. |
| `quayside-icon-256.png` | Preview. |

## Notes for the rename

- The icon follows Apple's macOS grid: an 824 px rounded square
  (corner radius 185) centred on a 1024 px canvas, with a soft drop shadow.
  The current icon fills the whole canvas, so it looks oversized in the Dock.
- To use it, replace `app/src-tauri/icons/icon.source.svg` with
  `quayside-icon.svg` and run `npm run icons` in `app/`.
- Colours: plate `#24324D` → `#0D1422`; crane `#B9DAFF` → `#4C93EC`
  (the in-app accent is `#58A6FF`); container `#FF9F4A` → `#E0612A`.
- At 32 px and below the stays and bracing blur together. If the Finder-list
  sizes need to be sharper, draw a simplified 16 and 32 px version (boom, legs
  and container only).
- Names checked on 2026-11-23: "Gantry" is taken by getgantry/gantry, a Docker
  manager for macOS, and Berth, Derrick, Stevedore and Bollard are used by
  container tools. "Quayside" had no clash among container tools, no Homebrew
  cask, and no repo under elementalcollision.
