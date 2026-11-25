# A repeated notice already in an overflowing notice stack is not scrolled into view

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

When the window's notice banners already fill their scrolling area, a warning that is already in the stack but no longer
last is not brought into view when the same event happens again. After a second loss of the helm connection, the
"connection to the helm was lost" warning can stay scrolled out of sight while the newest other notice shows. The same
applies to a second refusal to turn off upgrade highlights. During an outage the window itself still goes into its
reconnecting state, so a user who is watching sees the loss.

## Details

Pre-existing: before 23a68c91cb a rebuilt notice region also started at the top, so that commit does not make the case
worse, but it relies on the dedup in `notices::post`. 2b18554abb adds another notice (the upgrade-highlights opt-out
refusal) that can hit it.

Locations: `crates/gui/farhelm-gui/src/main_view.rs:204-207` and `:260`;
`crates/gui/farhelm-gui/src/notices.rs:177-186`; `crates/gui/farhelm-gui/src/session.rs:220-222`;
`crates/gui/farhelm-gui/src/whats_new.rs:105-110`.

`notices::post` (`notices.rs:180`) ignores a notice already posted, so on recovery `session.rs:221` re-posting
`Notice::ConnectionLost` leaves it at its old position, and `whats_new.rs:108` re-posting
`UpgradeHighlightsOptOutFailed` does the same. Any notice posted after the first instance (an interrupted upload, Not
launched, the connection notice itself) stays below it. 23a68c91cb clears `drawn_banners` while inert
(`main_view.rs:260`), so after a reconnect `reveal_latest` is true and `notice_banners` scrolls to the bottom
(`main_view.rs:463-465`); for a repeat while connected, `reveal_latest` is false because the last posted notice is
unchanged (`main_view.rs:204-206`). The repeated warning can sit above the visible part of a stack capped at half the
window (`max_h relative(0.5)`, `main_view.rs:474`).

The opt-out route: if Don't show again is refused, the window reconnects (posting the connection notice below it), the
highlights reappear (SPEC.md:5003-5004 allows that after a failed save) and the opt-out is refused again;
`notices::post` ignores the repeat, and the refusal stays where it was.

SPEC_impl.md:1435-1436 says "New banners scroll into view". A repeat is not a new banner, so the spec does not strictly
require this, but SPEC_impl.md:476-478 makes the connection notice how the user learns an action may or may not have
happened. Mitigation: during an outage the window goes inert behind "Reconnecting to the helm…", so a user watching sees
the loss.

Fix: have `post` move a repeated notice to the end (withdraw, then push) so it becomes the latest and is revealed, at
least for the unit notices `ConnectionLost` and `UpgradeHighlightsOptOutFailed`.

Bucket: other (a warning present but scrolled out of view, only with an overflowing stack and an undismissed earlier
instance above newer notices; the inert reconnect surface already signals the outage). Not highest: no data or security
consequence is established, and the notice is reachable by scrolling.

Confidence: confirmed from source for the ordering and scroll behavior.

Possible cover: SPEC_impl.md:477, "One generic notice says the connection was lost", may be read as accepting a single
deduplicated notice.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g15 record f1; claude-opus-5-5
reviewer, source analysis only, nothing executed.
