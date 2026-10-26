# Window checks inherit another attempt's image hashes

Reviewed commit: b1eed46975aab16ccc2217b799d5d9a045d8e979

## TLDR

The window-drag check validates the whole pass's frame history against only its own event stream, so prior preflight
images can reject otherwise valid evidence. Actual captures and frequency are unmeasured; release-test availability is
the consequence.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `releasing/mac-vm-test/guest_operations.py:372-386`. Locations and revisions in the assessment below
identify the reviewed source; later main is outside this review's pinned range.

Reviewed commit: b1eed46975aab16ccc2217b799d5d9a045d8e979. TLDR: A successful bonus preflight can make the later
fresh-pass window-drag release check reject valid evidence. The host uses one pass_directory across native-preflight and
later checks (brick_test.py:734-755,758-783), with distinct attempts (guest_workflow.py:512-524). Operations loads
pass-wide managed-state.json (guest_operations.py:66-70); window_frame appends to pass-wide window_frames and writes its
entire history into the current attempt artifact (372-386), and receipt saves the shared state (92-107). Successful
bonus preflight supplies image-associated Finder measurements (guest_workflow.py:305-318; guest_tester.py:301-305;
window_geometry.py:125-141). Fresh later requires window-drag (brick_test.py:211-213). Its geometry validator filters to
Farhelm (window_geometry.py:96), but provenance validation checks every accumulated image against only current-attempt
native events (guest_tester.py:509-518). Any preflight Finder hash absent from the ten current Farhelm images rejects
the otherwise passing checkpoint, producing automation-failure (guest_tester.py:775-796). Suggested bucket: other,
confined to maintainer release-test availability/evidence. Confidence: confirmed conditional source mechanism. Caveats:
source inspection only; actual capture bytes/frequency unmeasured. No product failure or broken release promotion
claimed. possible_cover: none found in pinned SPEC.md, SPEC_impl.md, empty TODO Planned bucket, BUGS.md, queue INDEX or
TRIAGE_OUTCOMES searches. SPEC_impl.md:926-977 concerns product drag behavior; ledger
replaced-evidence-repeated-continuity-receipts-retain-passing.md concerns validating and hashing the same bytes, not
cross-attempt trace provenance. Neither covers this trigger/consequence. No FILTER match: the producer is normal
successful bonus preflight, not a rare interruption or unsupported input. proposed_drop: none. The trace/provenance
scope must agree on which attempt owns each image; keep this distinct from g311/f1 screenshot export budgets and g312/f1
provider binding semantics.

Provenance: `g310/f1`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.
