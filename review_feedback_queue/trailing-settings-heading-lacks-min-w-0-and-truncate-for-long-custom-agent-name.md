# Settings page heading lacks min_w_0() and truncate(), allowing long custom-agent display names to push the scope label off-screen or bloat the pinned header

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

Opening a custom agent with a long display name in Settings can push the page's scope label ("Every host on this helm")
past the right edge of the dialog or expand the pinned heading vertically over multiple lines, reducing space for the
scrollable form.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/settings/frame.rs:436`.

Commit df0dff4c9b925534b0aae763c325155279a58829 changed `SettingsDialog::page_heading`
(crates/gui/farhelm-gui/src/settings/frame.rs:385-389,429-448) so `#settings-heading` now renders arbitrary
user-authored `CustomAgent.display_name` (which `CustomAgent::validate` in
crates/farhelm-proto/src/custom_agents.rs:25-33 does not length-cap) via `section.display_label(cx)` (`HostSentence`),
rather than only short static section names. Unlike the sidebar nav row (`settings/frame.rs:299`:
`.flex_1().min_w_0().truncate()`), the description directly beneath it (`settings/frame.rs:455`: `.truncate()`), and the
search heading (`settings/search.rs:735`: `.truncate()`), `#settings-heading` (lines 436-446) has neither `.min_w_0()`
nor `.truncate()`, and its parent flex row (lines 430-434) has no `.min_w_0()`. As `host_text.rs:587-590` documents for
GPUI, a flex item without `min_w_0()` takes its full unwrapped text width as its minimum width, so `#settings-heading`
refuses to shrink inside the flex row beside `.flex_none()` `scope_line`, pushing `scope_line` off the right edge of the
760px dialog for long display names; and without `.truncate()`, multi-word long names grow the `flex_none()` pinned
`heading_band` vertically. How to verify: inspect settings/frame.rs:429-458 vs settings/frame.rs:299 and
host_text.rs:587-591,1090-1116 (which verifies `.truncate()` clips a child `HostSentence` cleanly with an ellipsis).
Fix: add `.min_w_0()` to the heading flex row (line 430) and `.flex_1().min_w_0().truncate()` (or
`.min_w_0().truncate()`) to `#settings-heading` (line 436).

Reviewer metadata (g51_custom_agents_gui_settings_b/f6): suggested bucket: other (layout overflow / clipping of the
Settings page heading and scope label for long custom-agent display names; no security or data-loss impact).;
confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g51_custom_agents_gui_settings_b/f6`.
