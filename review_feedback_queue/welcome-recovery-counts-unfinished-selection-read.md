# Welcome recovery counts an unfinished selection read

Reviewed commit: 56dd4d2d9a9d16a182256c73d8aad2c9e06188e9

## TLDR

The welcome-recovery browser test can fail on an extra legitimate preference read if authentication recovery interrupts
initial session selection. The adverse scheduling has not been reproduced.

## Details

`e2e/tests/welcome.spec.ts:113–116` increments its counter on handler entry, before response completion. The new
exact-two wait (`134`) proves the automatic-selection request started, not that selection finished. The staged listing
401 and authentication recovery (`139–150`) then expect exactly three reads (`153`), assuming one reseed only.

Selection starts absent (`crates/farhelm-ui/src/lib.rs:1602`); authentication replaces the preference gate/list subtree
(`1710–1719`). Automatic selection awaits its GET before opening a session (`list/view.rs:2977–3012`). If that read is
pending at unmount, reseed is read three and the unselected remounted list legitimately performs read four. The
assertion can fail on four or transiently pass at three. Observe completed initial selection before inducing recovery
when testing one reseed, or deliberately stage the interrupted-selection case and accept its extra read.

Suggested bucket: **other**. Confidence: **likely**, with static request-start/remount boundaries established and the
timing/count observation unverified. No security or user-work loss was established. Proposed drop: none.

Possible cover: final `SPEC.md:2727–2741` and `SPEC_impl.md:172–184` require welcome state to survive recovery, not
three total reads; `210–226` requires the extra pre-pick GET. No exact Planned, BUGS, queue or ledger match was found.
FILTER does apply to committed-test review and might cover a rare retryable failure, but rarity and complete match
remain unestablished. Provenance: `session-history-tests-1/f5`, discovery pass 1, trailing review through
`fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`. No runtime reproduction was performed.

Original finding IDs: `session-history-tests-1/f5`.
