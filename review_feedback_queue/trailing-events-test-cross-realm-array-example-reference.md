# Cross-realm test comment points to an example that does not exist

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: Cross-realm test comment points to an example that does not exist

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g185/c1`.

### g185/c1

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 5 (finding, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e

## TLDR

The event-feed test sends future maintainers to a paste-sanitizer test for a cross-realm array technique that the
referenced test does not use. Its explanation of the test harness now contains a false example.

## Details

The commit's actual patch changes events.test.js:145 from term-bytes.test.js to paste-sanitizer.test.js while retaining
the claim that the referenced file uses Array.from to rebuild a value in the assertion's own realm.
events.test.js:140–150 explains that objects produced by node:vm have a different Object.prototype and reconstructs each
report with a local object spread. The complete paste-sanitizer.test.js at this revision (lines 1–60), reread during
synthesis, contains no Array.from, cross-realm array reconstruction, or structural comparison of a sandbox-created
array/object. Its VM test at lines 46–59 calls sanitizePastedText and compares only a function's typeof and a returned
primitive string.

Historical attribution was also checked directly:
f560058da19e02716e6f943883a9508b447aff4e^:crates/farhelm-ui/js-tests/term-bytes.test.js:108–116 actually explains the
cross-realm Uint8Array problem and calls Array.from on the sandbox result. Thus the old reference was accurate; the
assigned patch introduces the false reference by pointing that same claim at the extracted sanitizer-only test. This is
a materially false example, not merely absent explanatory detail. Remove the specific Array.from/example assertion, or
point it to a real test that demonstrates that technique; the existing local explanation of the report reconstruction
can remain. The other assigned reference changes claim only the CommonJS/browser dual install or the node:vm
actual-source pattern, both of which the sanitizer asset/test still demonstrate; source checks confirmed those exports
and the VM test.

Suggested bucket: other (documentation correctness). Confidence: confirmed by patch attribution, parent source and
complete referenced-file source, not execution. proposed_drop: false. The initial c1 event proposed omitting this
because it changed no runtime contract or user behavior and the local explanation was self-contained; that was a
severity/actionability filter, not source refutation, and is superseded. Preserve its original consequence: misleading
navigation for readers, with no established runtime failure or security/loss-of-work route. Could-be-highest: false.
possible_cover: none; no coverage sources were searched. No reproduction or test execution was performed. No premise
remains unresolved.

Returned proposal (claims-116, event 5): **retain-verified**, suggested bucket **other**.

The reviewed patch replaces an accurate example reference with a file that does not demonstrate the claimed technique.
This is an in-scope source-comment error, with misleading navigation for maintainers and no established product failure.

Original stronger consequence: No stronger consequence was asserted. Initial line 1 proposed omitting the misleading
example because it changed no runtime contract and the local explanation was self-contained. Lines 3 and 5 explicitly
supersede that proposed omission; the established consequence remains misleading maintainer navigation, without a
security or user-work-loss route.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Record an other-priority documentation-correctness finding. The source establishes the false example and
its introduction within the range. No exact coverage or filter applies; runtime correctness does not refute the comment
defect.

Existing parent disposition (event 5): verified. Parent explicitly accepted named retain-verified proposal from
claims-116 after inspecting it. The reviewed patch replaces an accurate example reference with a file that does not
demonstrate the claimed technique. This is an in-scope source-comment error, with misleading navigation for maintainers
and no established product failure. Record an other-priority documentation-correctness finding. The source establishes
the false example and its introduction within the range. No exact coverage or filter applies; runtime correctness does
not refute the comment defect. Checked source/results:
["f560058da19e02716e6f943883a9508b447aff4e:crates/farhelm-ui/js-tests/events.test.js:140-150 explicitly says
paste-sanitizer.test.js uses Array.from for the same cross-realm workaround; the patch changes only the referenced
filename at line 145.","Read complete
f560058da19e02716e6f943883a9508b447aff4e:crates/farhelm-ui/js-tests/paste-sanitizer.test.js:1-60. Its VM test compares
typeof and a primitive string; it contains no Array.from or sandbox-array structural
comparison.","f560058da19e02716e6f943883a9508b447aff4e^:crates/farhelm-ui/js-tests/term-bytes.test.js:108-116 explains
cross-realm Uint8Array prototypes and uses Array.from, establishing that the filename replacement introduced the
error.","Explicit ancestor checks place f560058da19e02716e6f943883a9508b447aff4e after the review base and before the
review tip. No runtime test or reproduction was run."] Current coverage/results: ["SPEC.md and SPEC_impl.md: searched
cross-realm, Array.from, paste-sanitizer, term-bytes and related shortcut/terminal-input terms; no accepted rule covers
a false test-example reference.","TODO.md: Planned is empty at lines 25-26.","plans/queue: searched the actual
filenames, cross-realm and Array.from; none match. Read tmux-drop-from-docs.md acceptance criteria 3 and 5 in full: its
sweep addresses tmux/xterm comparisons and related complexity, not this now sanitizer-named cross-realm reference. The
broader sweep does not explicitly remove this cause. tmux-end-old-terminals.md criteria 4-5 describe the already
implemented removal, not an unlanded repair for the false reference.","BUGS.md and active TRIAGE_OUTCOMES.md: searched
the actual filenames and cross-realm/Array.from terms; no match. The archive was not read.","Read
review_feedback_queue/INDEX.md in full and searched queue files for cross-realm, Array.from, paste-sanitizer and
term-bytes. The only Array.from hit concerns sandbox shard allocation and is unrelated. No same-root queue alias
found.","FILTER.md: no filter covers an ordinary persistent false source-comment reference. The initial
severity/actionability omission is not a listed filter."] Exact cited cover: [] Original stronger consequence and
missing premise remain preserved: "No stronger consequence was asserted. Initial line 1 proposed omitting the misleading
example because it changed no runtime contract and the local explanation was self-contained. Lines 3 and 5 explicitly
supersede that proposed omission; the established consequence remains misleading maintainer navigation, without a
security or user-work-loss route."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g185 record](records/review-through-00de0b6c/groups/g185.json). Record ID: `c1`.
