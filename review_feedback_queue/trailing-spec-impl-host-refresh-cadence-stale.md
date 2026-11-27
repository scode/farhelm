# SPEC_impl.md's connection-manager cadence bullet still states the old three-second session-list refresh

Reviewed commit: c2c9a634018f1cad19680dcf2602dd76c084562a

## TLDR

The implementation spec now contradicts itself about how often the helm re-reads a connected host's session list. One
section says thirty seconds (backstop only), another still says every 3 seconds 'matching the UI's own poll interval'. A
reader (or agent) checking behavior against the spec gets the wrong number and the wrong rationale.

## Details

Commit c2c9a634 changed manager::REFRESH_INTERVAL from 3s to 30s (crates/farhelm-helm/src/manager.rs:171) and rewrote
the hint paragraph (SPEC_impl.md:3218-3226) and the ntfy paragraph (SPEC_impl.md:6338-6354), but left the Cadences
bullet at SPEC_impl.md:5846-5853 unchanged: 'A connected host's session list refreshes every 3 seconds, matching the
UI's own poll interval, so multi-host aggregation does not make the visible list staler than the single-host path
already is.' That sentence is now false on both the number and the reason (the poll is a backstop; hints carry
freshness). Verify: grep SPEC_impl.md for '3 seconds, matching the UI'. Fix: rewrite the sentence to say the connected
host is refreshed on each change hint and by a thirty-second backstop poll, pointing at the hint paragraph. AGENTS.md
treats SPEC_impl.md as authoritative, so the stale line is a contract statement, not a comment.

### Triage notes

- Recorded bucket: `other`.
- Suggested bucket (reviewer): other (documentation contradiction, no runtime effect).
- Confidence: confirmed.
- possible_cover: none
- proposed_drop: none

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g26/f1 (finding), group g26 "Polling each host's session list every 30 seconds", findings log line 1.
