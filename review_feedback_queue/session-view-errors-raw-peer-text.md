# Session-view error lines show refusal text raw

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A failed restart or replace can show an error whose wording or named session is visually scrambled by characters a
remote host inserted, misleading the user about what still exists.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as `F29 / SEC-VIEW-ERRORS-RAW`,
tagged **possible**. Anchors and title: `crates/farhelm-ui/src/session_view.rs:1729`,
`crates/farhelm-ui/src/session_view.rs:1732`, `crates/farhelm-ui/src/session_view.rs:1946`,
`crates/farhelm-ui/src/session_view.rs:1952`, `crates/farhelm-ui/src/list/view.rs:2602` — Session-view error lines
render helm and supervisor refusal text raw, bypassing peer escaping

When a mutation fails, `api::refusal_text` returns the helm's error body unaltered. Its doc says so explicitly and says
that the surfaces displaying it escape and isolate it with `peer::PeerLine`, because a refusal can quote peer-supplied
text. The sidebar row does this (`row.rs:2241` and `:2255`, `PeerLine { parts: vec![DetailPart::Peer(err)] }`).

The session view does not. It renders `restart_error` (line 1729), `replace_error` (line 1732) and every per-tab
`tab-error` line (lines 1946–1952) as plain `"{err}"`. The listing's "failed to load sessions: {e}" line
(`list/view.rs:2602`) does the same. A tab-open refusal can include the dead shell's last output, which the supervisor
captures from the pane ("last words"), so that error text can be arbitrary terminal output. As with F27 and F28, this is
not script injection. The risk is that bidi or invisible characters reorder or hide part of the message. It matters here
because these lines report destructive outcomes: SPEC says a failed replace names both sessions and says whether the
source still exists, and the user cleans up based on that text. The suggested fix is to render them the way `row.rs`
does, e.g. `PeerLine { parts: vec![DetailPart::text("replace: "), DetailPart::peer(err)] }`, including the tab errors
and the listing failure line.
