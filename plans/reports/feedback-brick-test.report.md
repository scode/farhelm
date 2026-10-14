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

### Landing

Landed as #2011, right after the round's other plan, launch-with-conversation-id (#2012), which does not touch
release-test tooling. Since the stack was based, main had also gained release advisories (#2010), which added a section
to releasing/AGENTS.md. The review checked that this plan's edits there and in the root AGENTS.md still fit the current
files; they touch different parts and do not conflict.

#### Release-relevant

Nothing ships to users. This changes the release brick test's procedure and coordinator only. The first live P1 feedback
send happens at the next authorized brick test.

#### Review and fixes made while landing

An independent fresh-context review found nothing blocking. It confirmed:

- P1 is refused until every P0 check has valid evidence;
- a P1 failure leaves P0's pass alone and does not block cleanup;
- a lost P1 success blocks guest deletion;
- stop-after-P0 sends nothing;
- the procedure keeps every mutation in the guest and never signs, publishes or touches the host's live install;
- the feedback carries only fixed text, a blank contact and the app's coarse version, surface and OS.

The landing fixed two of its nits:

- A stop-after-P0 requested late, after a P1 success was already recorded, bypassed the retention check, so cleanup
  could delete the guests behind a P1 success whose evidence had gone missing. That check now applies whenever any P1
  result is recorded. The existing lost-evidence test gained a case that narrows the scope to P0 after recording and
  asserts cleanup still refuses.
- Widening a run started as stop-after-P0 to stop-after-P1 would reach the live send without the send approval that
  run's preflight never asked for. BRICK-TEST.md now says to obtain that approval first.

#### Left as is

A run started by the older coordinator labels P0 priority differently, and no test exercises that compatibility path.
The reviewer found the code correct by inspection.

#### Checks

- Run now: the brick-test support suite through the recorder
  (`python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py'`): 82 passed, run
  `ce3887f3-eb55-429f-93fd-3290ffb07073`, and `dprint check` on the changed Markdown.
- Reused: the executor's focused P1 runs. The landing changed only the cleanup check, its test and one paragraph of the
  procedure.
- Not run: Rust, browser and website checks. Nothing they cover changed.
- The push of this PR failed once with no error detail and succeeded on the immediate retry.
