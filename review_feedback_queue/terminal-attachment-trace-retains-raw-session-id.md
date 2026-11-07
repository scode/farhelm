# Terminal attachment trace retains raw session id

Reviewed commit: 8f51f83ef0a304d7ef568bae30ad98e892ac7c6c

## TLDR

Terminal attachment diagnostics retain a raw peer-supplied session identifier instead of the promised cleaned
presentation copy.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-helm/src/terminal.rs:132-155 at 8f51f83ef0a304d7ef568bae30ad98e892ac7c6c`.

Source assessment (read with the later corrections below) (review record g077/l2, event 13):

Reviewed commit: 8f51f83ef0a304d7ef568bae30ad98e892ac7c6c. TLDR: invisible/bidi characters admitted in a hostile host's
session ID remain raw in terminal-attachment debug logs. Existing source basis remains terminal.rs:132,142,155,776;
sessions.rs:764-802; proto/session_ref.rs:98-113; session_cache.rs:46-47; UI api/http_contract_tests.rs:26-41. Controls
for ANSI execution are rejected, but Cf characters survive the 40-byte opaque suffix admission and normal browser route.
SPEC_impl.md:3075-3077 at the reviewed commit requires peer identifiers and successful attachment fields to be
normalized; the session-ID logging section at :144-152 independently forbids formatting disguise. Pinned Cargo.lock
selects tracing-subscriber 0.3.23; exact-version fmt/format/mod.rs:1299-1339 emits ordinary named Debug fields without
EscapeGuard, so %session remains raw. Suggested bucket other; confidence confirmed for emitted text, viewer-dependent
visual effect; no security/data-loss route claimed. proposed_drop false; possible_cover unknown except corroborating
duplicates supplied by parent. Interface synthesis: g006:g006-l2 at g006/findings.jsonl:6 and g030:f8 at
g030/findings.jsonl:21 both explicitly preserve g077-l2 ownership and narrow their refutations to repaired tab controls.
Their duplicate reporting is not new discovery and does not refute this sibling session-field defect. g030:f8 also
expressly excludes stored terminal-emulator ignored-format behavior from any log-display exemption. Scope and temporal
qualification remain unchanged: the raw session field predates this partial repair and is unchanged through
8a8c9579e04299410652f0e4681975a7198dfc7b. Fix remains trace-boundary normalization for both lifecycle events with
deliberate desktop-smoke compatibility. No runtime or viewer experiment performed; no other group inventory inspected.

Later qualification (review record g077/l2, event 35):

Reviewed hash 8f51f83ef0a304d7ef568bae30ad98e892ac7c6c. TLDR remains: with attachment debug tracing on, a hostile host's
admitted session suffix can put invisible/bidi formatting in both lifecycle log events. terminal.rs:132 stores
session.to_string(), :142/:155 logs Display, :776 supplies the ordinary decoded HTTP reference.
proto/session_ref.rs:98-113 rejects C0/C1 controls/overlong/dot suffixes but permits U+200B/U+202E; sessions.rs:764-802
retains the admitted bytes. UI api/http_contract_tests.rs:26-41 already proves encoded terminal-path retention. Supplied
neighbor g027:g027-d4@38 adds production provenance: session_view.rs:1638,1685,1712 and tabs.rs:256-267 use the raw ID
in percent-encoded route builders; terminal.rs:504-509,522-527 decodes both routes. Preserve that record's owner g027,
ledger source scope d81a3980548e354bcf2ff8a06d28235722f15ba4 and implementation scope 8f51; its residual finding is a
duplicate of g077-l2, not a new local discovery. Raw protocol storage is correct and does not excuse raw diagnostic
emission: normalize this owned trace copy as the patch does for tab. manager.rs:732-762 returns a fresh bounded
Debug-escaped String. Exact pinned tracing-subscriber 0.3.23 named-field visitor does not sanitize this Display value.
Suggested bucket other, confidence confirmed source-derived bounded diagnostic ambiguity, viewer effect not dynamically
observed; ESC/newline admission, execution, credential/data loss are specifically not claimed. proposed_drop false for
l2; possible_cover includes supplied g006:g006-l2, g030:f8 and g027:g027-d4 corroboration only, with parent owning
actual coverage. No terminal.rs/session_ref.rs repair through inclusive tip 8a8c9579e04299410652f0e4681975a7198dfc7b.
d4's wording withdrawal is not a refutation of this field defect. No neighbor inventory/ledger/queue search or
runtime/browser/process/network experiment.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g077/l2`; supporting records
`g027/g027-d4`, `g006/g006-l2`, `g030/f8`. The retained private log preserves earlier wording and its corrections. This
record does not claim runtime reproduction.
