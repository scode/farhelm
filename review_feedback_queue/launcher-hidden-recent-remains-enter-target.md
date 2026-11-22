# A scrolled-out recent setup can still take Enter while only the generic launch cue stays visible

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

A recent setup can scroll out of view while Enter still uses it to submit the launcher.

## Details

**high.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d:crates/gui/farhelm-gui/src/launcher/view/search/start.rs:107–146,154–211,267–305,522–541;
crates/gui/farhelm-gui/src/launcher/view/search.rs:245–260,607–625,647–655; SPEC.md:1427–1439,3625–3629,5572–5581;
SPEC_impl.md:8943–8948

When scrolling clips the highlighted recent row, its setup and its row-specific Return/Replace cap disappear. The
command field still says Enter on empty launches, and Replace with still has its general delete-source explanation and
footer. Nothing outside the clipped list says Enter instead submits that particular saved setup. start.rs:154–155
intentionally retains keyboard highlight while scrolling, and :206–210 only reveals on a changed highlighted child
index. The spec also lets wheel input preserve focus and not count as moving on, but SPEC.md:5572–5581 separately
requires using the shown action/target or ignoring an action whose target is no longer in view. Keep the selection as
designed, and refuse submission when it has no presented row or equivalent visible cue. This is a changed command-pane
route, distinct from LC-03’s inherited clipping of process-warning text in the footer. The reviewer suggested high for
unintended settings; the visible mode explanation and footer still identify source deletion, so no independent highest
claim from clipping alone. Confidence confirmed from source; no runtime scrolling checked; ; Possible coverage: shared
presentation fix with recent-enter-drawn-binding.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`launcher-match-actions/offscreen-recent-remains-enter-target`, with `launcher-command/LC-07`.
