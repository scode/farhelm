# Browser-test exit cleanup signals process numbers retained across long waits

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Long-running browser-test cleanup could signal an expired fixture's replacement.

## Details

F75 — **possible** — `scripts/test-start-stack-cleanup.sh:90–99`; `scripts/test-start-stack-cleanup.sh:99` —
Browser-test exit cleanup signals process numbers retained across long waits

Exit cleanup retains fixture process numbers across a multi-phase run. The decoy's fixed lifetime can expire well before
teardown, leaving its old number as signal authority. If the run outlasts it and that number is reused, cleanup could
terminate unrelated work; no elapsed-time or reuse sequence was reproduced. Tie signals to owned process instances and
make the decoy's lifetime controlled by its owner.

## Evidence and triage context

- scripts/test-start-stack-cleanup.sh:83–84 starts sleep 300 and saves its PID. Lines 90–99 signal saved fixture
  numbers, including an unconditional decoy signal. Readiness at :118–140 and the three phases at :303–354 allow
  substantial later work without an overall decoy-lifetime guarantee.
- scripts/test-start-stack-cleanup.sh:83-84 launches sleep 300 and saves its PID.
- scripts/test-start-stack-cleanup.sh:115-147 permits substantial readiness waits for each stack.
- scripts/test-start-stack-cleanup.sh:305,322,339 boots three successive stacks.
- scripts/test-start-stack-cleanup.sh:90-103 installs final cleanup that unconditionally signals the saved decoy PID at
  line 99.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- SPEC_impl.md:2201–2202 excludes numbers stored for later or carried across asynchronous work. TODO.md:241–253 concerns
  lingering test stacks, not unrelated-process signaling.
- SPEC_impl.md:2194-2205 accepts reuse within at most a few seconds but explicitly excludes storing a bare number for
  later. TRIAGE_OUTCOMES.md:6523-6528 repeats that exclusion.

Caveats:

- Requires fixture exit and subsequent PID reuse; not reproduced.
- Requires the run to outlast the decoy and its PID to be reused before cleanup.
- No elapsed-time or PID-reuse reproduction was performed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_10_cor:p1:F5`,
`automation_website_10_sec:p1:F6`.

- `automation_website_10_cor:p1:F5`: confidence as filed: possible; suggested bucket as filed: highest.
- `automation_website_10_sec:p1:F6`: confidence as filed: possible; suggested bucket as filed: highest.
