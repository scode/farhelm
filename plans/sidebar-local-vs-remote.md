# Local versus remote in the sidebar: a kind icon and the host on the title line

Anchor: commit `1e4a1340fa29ddc65b3373feb80e5a2432e78e82`, 2026-09-03.

Covers the "make local versus remote obvious in the sidebar" entry under "near term" in TODO.md. Effort: low. That rests
on the change being confined to one row component, one stylesheet section, a new icon module of two glyphs, the host
panel picking the same icon up, and two spec paragraphs; the browser suite already asserts the host line and needs its
selectors kept rather than rewritten.

NOTE: a plan, written against the anchor commit; check `git log <anchor>..main` over the paths below before executing.
The host-alias entry that follows this one in TODO.md changes what NAME the slot designed here shows, not where it sits,
so this plan should land first and the alias work should reuse its slot.

## Decisions already taken

Settled with the maintainer on 2026-09-03:

- The icon is inline SVG, two glyphs, drawn in `currentColor` and embedded in the rsx. No icon font, no asset files, no
  emoji. The app has no icon vocabulary today (the only glyph anywhere is the "⋯" menu toggle), so this is its first
  entry; keep the module shaped so the tooltip and alias work can add to it.
- Every row carries the icon: local rows the local glyph, remote rows the remote glyph. Local is a positive signal, not
  an absence to notice.
- The host name moves up to the title line, AFTER the title and its badges and BEFORE the age, so the title ellipsizes
  first and the host keeps a bounded share of the line. The second host line goes away.

```
● Fix the parser…      ⧉ devbox   3m
  ~/git/farhelm            claude
```

The exact glyph shapes are an implementation choice (a monitor or laptop for local, a server or a cloud-with-arrow for
remote); what matters is that they read at 12 to 14px in both themes and are visibly not the same shape.

## What changes for the reader of a row

Today a row shows the host only when the session is not on the helm's own machine, on a 12px muted second line, and a
local row shows nothing at all about locality. SPEC.md's Session list section states that rule and SPEC_impl.md's
sidebar-row paragraph records why it was chosen on 2026-08-23 (a host word repeated on every row of a mostly-local fleet
cost a quarter of the row's height to say nothing). This plan keeps the height argument and reverses the visibility one:
the icon says local or remote on every row at the cost of one glyph's width, not a line, and the remote name rides the
title line rather than its own.

The "this machine" wording stays the helm's rendering of the local host (`aggregate::host_display_name`), and the row
still never prints it: a local row shows the local glyph and no name. The name slot is for remote rows.

### Locality has three answers, and the icon must respect the third

`list::shared::session_is_local` answers a boolean, and its docs explain why both unknowns (an old helm sending no host
id, a hosts read that has not landed) answer "not local": unknown locality must never suppress a host name the row
already has. That rule survives, and the icon adds one more: unknown locality must not draw the LOCAL glyph, because
that would be inventing the claim the boolean was designed not to make. So the predicate becomes three-valued (local,
remote, unknown), an unknown row shows its name and NO icon, and the existing flicker (a host line appearing on first
load and vanishing once `/api/hosts` answers) becomes an icon and name appearing in the slot instead. The browser
suite's existing assertion that `.session-host` reads "this machine" during that window (sidebar.spec.ts around
line 2164) is exactly this case and should keep passing with the name in its new place.

## Files

### `crates/farhelm-ui/src/icons.rs` (new)

Two components, `LocalHostIcon` and `RemoteHostIcon`, each an inline `svg` with a fixed `viewBox`, `fill`/`stroke` of
`currentColor`, `aria-hidden="true"`, and a class (`host-kind-icon`) the stylesheet sizes. The module doc says why SVG
and not a font or emoji (platform font variance across WebKit and Chromium, and the desktop asset handler's fixed asset
set, which an icon file would have to join in both bundles under `scripts/check-desktop-assets.sh`). The accessible word
is NOT inside the icon: the row renders it as a visually-hidden span beside the glyph ("local" / "remote"), the same
clip-not-remove pattern the status badge uses, so a screen reader hears the locality and the suite can assert it.

### `crates/farhelm-ui/src/list/shared.rs`

`session_is_local` becomes `session_locality` returning a three-variant enum; the boolean callers (the row, the create
form's host default if it reads it) map through it. The doc comment's argument about unknowns is kept and extended with
the icon rule above. Unit tests: the existing ones re-expressed on the enum, plus one asserting an unknown host id or an
unlanded hosts read yields `Unknown`, not `Remote`.

### `crates/farhelm-ui/src/list/row.rs`

The title line (`.session-row-line` holding `.session-title`, the stale and archived badges, the status badge, and
`.status-time`) gains, between the badges and the age, one slot: the icon for a known locality, then for a remote or
unknown row the name in the existing `.session-host.peer-value` span with `dir="ltr"` and `display_peer` escaping, kept
exactly as it is today because it names the machine a stop or delete will reach (the comment on the current host line
explains the direction-isolation reason; move it with the span). The second-line host block is removed. Add a
`data-host-locality` attribute (`local`/`remote`/`unknown`) on the row for the suite, the way `data-host-kind` serves
the host panel.

Width contention is the one design risk on this line. Today it holds the title (`flex: 1 1 auto`, ellipsizing), two
optional badges, a 7px dot, and a fixed-width age; the host name is a second ellipsizing claimant. The rule: the title
keeps `flex: 1 1 auto` and the host gets `flex: 0 1 auto` with a `max-width` of about 40% of the line, so a long
destination cannot push the title to nothing and a long title cannot hide the host entirely. The stale and archived
badges and the age never shrink. Check the narrowest sidebar width the layout allows (the sidebar's own `min-width` in
app.css) against a remote row with both badges.

### `crates/farhelm-ui/assets/app.css`

`.session-host` leaves the `.session-cwd, .session-invocation, .session-host` shared rule for one of its own on the
title line: 12px, muted, `flex: 0 1 auto`, `max-width: 40%`, ellipsis. `.host-kind-icon`: `width`/`height` matching the
title's cap height (12px), `flex: 0 0 auto`, vertically aligned to the text baseline the way `.status-dot` is (the
comment at the `.visually-hidden` rule explains why the baseline must come from a visible element). The icon inherits
the row's text colour, so a stale row's dimming and a selected row's foreground apply without a rule of their own.

### `crates/farhelm-ui/src/hosts.rs`

The host panel's row (`.host-name.peer-value` with `data-host-kind`) gets the same icon before the name, from the same
module. Not asked for by the entry, but two renderings of "this is a remote host" with two different vocabularies would
be the inconsistency the shared module exists to prevent, and it is a one-line addition.

### `crates/farhelm-ui/src/session_view.rs`

The open session's header builds its host wording from the row's `host_name` (around line 1327). Unchanged by this plan;
noted so nobody looks for a second host line to remove there.

## Spec and docs

`SPEC.md`, Session list: the row paragraph ("The host is named on its own line only for a session that is not on the
helm's own machine …") is rewritten to say every row marks whether its session is local or remote, and a remote session
also names its host on the title line, with the same "full value on the row" tooltip promise as the directory.
`SPEC_impl.md`, the sidebar-row paragraph (around line 110) records the 2026-09-03 reversal beside the 2026-08-23 one:
what was reversed (visibility, on every row), what was kept (no extra line), and the three-valued locality rule with the
reason the local glyph is withheld on unknown.

## Browser tests

Existing assertions on `.session-host` (sidebar.spec.ts lines 434 and 2164 to 2168, terminal-multihost.spec.ts lines
1800 and 1897) keep their selectors; the only expectation that changes is WHERE the span sits, which none of them
assert. New cases in sidebar.spec.ts: a local row carries `data-host-locality="local"`, the local icon, and no
`.session-host`; before the hosts read lands (the existing "this machine" case) the row carries `unknown` and no icon;
and the multihost spec's remote row carries `remote` with the remote icon beside the name. A width case that pins a long
remote destination not collapsing a long title to nothing is worth one assertion on the title's rendered width being
non-zero.

## PR shape

One PR. The icon module, the row change, the stylesheet, the host panel line, and both spec paragraphs belong together
because half of them would contradict the other half if split; the entry and this file are removed in it.
