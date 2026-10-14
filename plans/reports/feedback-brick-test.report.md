### What this was about

Release validation did not prove that the candidate app could submit feedback to the separately deployed live service.
The maintainer chose one real GUI submission immediately after the five critical installation, upgrade and agent checks
(P0), using their last Mac guest. This adds that separate P1 feedback check to the release brick-test procedure and
coordinator. It does not run the brick test itself.

### Things you should know

Full and stop-after-P1 runs now retain the final P0 guest for the candidate's feedback submission. The message starts
with `[brick-test] vX.Y.Z`, followed by a fixed explanatory sentence; the contact is blank. The operator verifies the
displayed candidate version and desktop surface, retains filled and completed-dialog captures, and accepts success only
when the dialog closes by itself without a failure. Existing GUI evidence and operator attestation remain the mechanism.

P1 requires valid evidence for all five P0 checks, including when recording failure or an unavailable check. Its verdict
is reported separately and included in the full verdict. A failed or unavailable P1 does not erase P0's pass, prevent
ordinary cleanup or block bonus checks. A recorded P1 success whose evidence later disappears or changes preserves the
guests until that evidence is recovered. Stop after P0 still sends nothing; stop after P1 finishes both categories and
cleanup. Historical runs without a declared P1 report it as not planned.

Permission preflight now includes the live send and resulting issue in the private inbox for scopes containing P1.
Whether a P1 failure blocks promotion remains the maintainer's decision for each release. No real machine profile,
guest, credentials or feedback endpoint was accessed during implementation; no feedback was sent.

### Open questions and possible follow-ups

The next authorized release brick test provides the first live P1 execution evidence. The local checks establish
coordinator behavior using synthetic observations; they do not claim a release or endpoint passed P1. No maintainer
decision blocks this implementation.

### The PRs

- [#2011: feedback brick-test procedure and coordinator](https://github.com/scode/farhelm/pull/2011/changes), one
  coherent draft PR, including removal of the covered TODO.

### Checks run, reused and skipped

- Run: recorded `python3 -B -m unittest discover -s releasing/mac-vm-test -p test_brick_test.py -v`, 39 passed, run
  `66e39bc5-8b85-4d8c-9618-01a6ac445bd0`; then the documented complete support suite with `-p 'test_*.py'`, 81 passed,
  run `005745e3-ae6b-43ba-89ff-9e1b0c10429e`. Both used
  `scripts/record-test-run.py --kind development --concurrency 'unittest serial' --tmux none` and explicit selections,
  with no skipped prerequisites.
- Run: recorded focused `test_brick_test.py -k p1 -v`, five passed after a reporting readability rewrite, run
  `b3d91580-3f27-4cfb-8775-3752284e4204`; six passed after the evidence-recovery correction, run
  `200767e7-bc2a-4e2f-b81f-3364f9282ff3`. The final selection covers lost-success recovery, ordering, separate verdicts,
  nonblocking failures, scope and historical obligations. No runtime failure occurred.
- Run: targeted `dprint check` on the five changed Markdown files and `git diff --check`, including after rebase. The
  separate documentation pass covered all seven touched files and the final correction.
- Reused: the full suite outside the corrected cleanup branch; the final focused run covers that correction. All Python
  behavior is unchanged by the careful rebase from `2638db29` to `48491e78`. Incoming release-notes menu work keeps Send
  feedback first and leaves its dialog unchanged; queue/report changes are bookkeeping. Both TODO removals are
  preserved. No further runtime repetition adds coverage for those independent changes.
- Skipped: Rust, JavaScript, browser, website, installer, hosted CI and live release tests, and the Rust/browser delay
  checker. This changes only Python release-test coordination and instructions, covered by the documented support suite
  and focused correction tests. No app, endpoint or release artifacts changed.

### Review gate outcome

The required fresh gpt-6.1-sol reviewer at high effort found one P2 defect: cleanup could delete guests after a recorded
successful P1 lost its evidence. The gate was corrected and a regression proves missing and changed P1-only evidence
refuse shutdown/deletion while P0 remains a pass, then restored bytes permit cleanup. The same reviewer independently
rechecked the correction and complete diff with the full test-authoring checklist and found no remaining issue. Source
review was read-only and did not independently execute tests.

Implementation stayed local under no-workhorse. Required source review used gpt-6.1-sol/high; fresh wording readers used
gpt-6.1-sol/medium, accepting the revised plain-language motivation. Report delivery receives the separate required
native own-model cold read. Private galaxy-brain records retain review evidence; native and orchestrator usage are
unavailable, and external counters are retained without inferring exact reported model or billing totals.
