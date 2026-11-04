# Repeated anonymous OSC starts split an honest address into a false mismatch warning

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

An honest hyperlink emitted through consecutive anonymous OSC 8 starts with the same destination can show a false
destination-mismatch warning and underline only its tail.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source plus
maintainer-recorded observation; no runtime reproduction.

Source locations: 7509285e647ea86ccd2cee7905675dc87dad9b6f:crates/farhelm-ui/assets/terminal.js:1268-1276.

terminal.js:1268-1276 passes only found.ranges[0] to linkHoverText; grid-buffer.js:286-287,300-320 separates
neighbouring runs when their ids differ even when their target matches. The helper terminal-links.js:467-510 then
compares the isolated tail when no adjacent row shares its tag. At inclusive tip
ca9b0a19856dc136f63ad2d14f1076027edce1eb, TODO.md:51-68 records this exact trigger, a concrete terminal-byte example,
observed real-agent symptom and proposed repair.

An honest hyperlink emitted through consecutive anonymous OSC 8 starts with the same destination can show a false
destination-mismatch warning and underline only its tail. Source: terminal.js:1268-1276 passes the one returned
contiguous range to linkHoverText; grid-buffer.js:286-320 joins only equal id-and-URI links; terminal-links.js:467-510
compares an isolated run when the tag has no adjacent-row continuation. Additional static producer verification at
pinned tip: vendor/alacritty_terminal/src/term/cell.rs:279-290 gives each anonymous hyperlink a fresh counter-based
_alacritty ID; crates/farhelm-supervisor/src/grid_output.rs:162-165 copies that ID and URI, and :707-712 keys the
emitted link on both. Thus the independently recorded exact producer is supported by source, not an unavailable runtime
premise.

Possible coverage and caveats: exact existing TODO.md Near term item A hyperlink a program starts twice shows a false
mismatch warning in grid terminals at pinned-tip :51-68; Planned is empty (:23-25), so do not silently apply Planned
coverage. Pinned-tip SPEC.md:2314-2332 and SPEC_impl.md:1671-1709 accept separate-tag warning limitations against
hostile split text but do not expressly accept this honest-address false alarm. TRIAGE_OUTCOMES.md
repeated-terminal-links-acquire-mismatch-warning.md :16915-16932 concerns repeated complete links sharing an explicit ID
on adjacent rows, not anonymous same-row restarts. Searches across the required coverage sources found no exact
scheduled plan, accepted BUGS entry, queue item or ledger outcome. FILTER.md's rare-glitch trigger is not established
for the observed ordinary agent producer.

Additional reviewed locations: g171/f3:
7509285e647ea86ccd2cee7905675dc87dad9b6f:crates/farhelm-ui/assets/grid-buffer.js:286-325.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g163/f3`, `g171/f3`.
Original append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs.
Locations refer to the named reviewed revisions; the original revision context is
7509285e647ea86ccd2cee7905675dc87dad9b6f; persists at ca9b0a19856dc136f63ad2d14f1076027edce1eb, with terminal.js,
grid-buffer.js and terminal-links.js unchanged between those revisions.. This was a source-only review; runtime
occurrence and frequency were not measured.
