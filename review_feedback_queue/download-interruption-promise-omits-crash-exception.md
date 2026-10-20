# Download interruption promise omits the accepted crash exception

Reviewed commit: b38bb6f12f34a5738d2f1331fae4c6240ab1d0b9

## TLDR

The product specification promises no partial download after an interruption, while the new cleanup policy explicitly
allows leftovers after a crash or force-quit. The conflicting wording could lead future work to promise or implement
cleanup that the maintainer has chosen to leave uncovered.

## Details

`SPEC.md:1865–1867` says desktop downloads leave "no partial file after failure or interruption", without limiting
interruption to a cancelled transfer or normal Quit. The new `SPEC_impl.md:86–103`, "Cleanup on quit", explicitly leaves
crashes and force-quits uncovered by default and requires specific approval before adding startup sweeps or
inter-process ownership machinery. Its preamble (`3–6`) says SPEC.md wins when the documents disagree about observable
behavior. The intended exception therefore has no matching qualification in the higher-priority product contract.

Suggested bucket: **other**; this is specification synchronization, with no security or user-work-loss consequence
asserted. Confidence: **likely**. The differing text is confirmed; the unverified premise is that SPEC.md's unqualified
"interruption" includes process death rather than only ordinary transfer interruption. No runtime defect or newly
required crash cleanup is claimed.

Possible cover: SPEC_impl.md's new section and `plans/queue/download-staging-sweep.md:309–312` explicitly accept the
leftovers themselves. SPEC.md's "Power-loss durability of terminal file downloads and checkouts" (`2920–2930`) exempts
saved downloads from durability guarantees but does not explicitly qualify the separate partial-file cleanup promise.
The existing `native-download-quit-leaves-partial-files.md` and its TRIAGE_OUTCOMES.md entry concern ordinary Quit, not
this wording conflict. No exact Planned TODO, BUGS, queue, ledger or filter coverage for the synchronization issue was
found. Proposed drop: none.

Reconcile the product wording with the accepted crash and force-quit exception, preserving normal-Quit cleanup and the
approval requirement. This finding does not ask for a sweep, locking, broader cleanup or reversal of that decision.
Verification is a comparison of the two contracts; product runtime tests would not resolve their wording.

Provenance: separate same-model adversarial reviewer of every individual commit in
`738598601c142e9e184ee7f0427e8995b2d89e0f..5c6e49937b1e779be2a5445059637aadf3757356`, followed by the commissioner's
inspection of the named specification sections and coverage sources.
