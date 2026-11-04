# Replay-order documentation describes the reverse of the supervisor contract

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The implementation notes misstate the acknowledgement/replay ordering that protects healthy terminal attachments from
false stalls.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source
contradiction.

Source locations: ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-helm/src/terminal.rs:767-769,1870-1878.

The implementation notes misstate the acknowledgement/replay ordering that protects healthy terminal attachments from
false stalls. This is a source-documentation defect; no terminal malfunction or user-work loss is established.
Details/source basis: At the introducing commit crates/farhelm-helm/src/terminal.rs:682-684 and :1617-1625 describe
replay preceding Attached as normal. Both explanations survive at the tip, at terminal.rs:767-769 and :1870-1878; the
test still writes data before Attached at :1900-1911. At the tip
crates/farhelm-supervisor/src/service/handlers.rs:1926-1957 enqueues Attached before spawning the tmux forwarder; the
host branch likewise enqueues Attached at :1965-1973 before spawning the grid sender at :1995-1997. Thus this is not an
intermediate-only statement repaired by the later grid relay. The later relay commit
3097bab8d322564f38f3f075103ebd184d106fe3 changes the capability gate and surrounding notices/tests but preserves the two
disputed explanations, as does 58094288259b0c29f74ea25936d4fe006c72128f. Distinguish wire enqueue order from caller
scheduling: frames can already be in the helm's event queue when the awaiting attach caller resumes, even when Attached
was processed first. Sending the notice before any data remains correct; the incorrect claim is that this requires or
ordinarily follows data before the acknowledgement on the supervisor connection. Correct that rationale and label the
intentionally reversed peer test as a robustness case rather than ordinary supervisor behavior.

Possible coverage and caveats: none found. Coverage sources were searched during the initial pass;
SPEC_impl.md:3016-3057 and :1706-1735 govern notice/data ordering, not replay-before-Attached; TODO.md Planned is empty;
TRIAGE_OUTCOMES.md:6048-6081 concerns hostile-peer pre-Attached frame bounds, not the false ordinary-case rationale. No
matching queue item, unlanded plan, BUGS entry, ledger decision or FILTER.md exclusion was found.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g037/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 5c39b2cd67629ecf7c80b0bc9a053144d1aac011.. This
was a source-only review; runtime occurrence and frequency were not measured.
