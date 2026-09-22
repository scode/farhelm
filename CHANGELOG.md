# Changelog

Notable user-facing changes in each stable release of Farhelm. Release candidates and dev builds are not listed; their changes appear under the stable release that follows them. Entries are written for someone running Farhelm, not for someone reading its source, so internal mechanics are left out unless they change what you have to do. cargo-dist copies each release's section into its GitHub release; `releasing/AGENTS.md` describes the format and how a section is written.

## v0.12.0 - 2026-09-21

### ✨ Highlights

#### The launch composer looks like the rest of Farhelm

The dialog that starts a new session had its own palette, corner radius, shadow, and selection color. It now uses the same ones as the main view, and selection is drawn the same way everywhere: the selected session in the sidebar, the selected terminal tab, and every chosen option in the composer share one look. Recent-setup rows show only the settings a setup sets explicitly, or "defaults" when it sets none, and line up as columns so the row that differs is easy to spot. (#819, #820)

### 🔄 Changed

- The launch composer uses the main view's colors, corners, and selection style, and recent-setup rows show only what each setup sets. (#819, #820)

### 🔧 Fixed

- Buttons, inputs, and selects in the launch composer render in the UI typeface instead of the platform's sans-serif. (#818)
