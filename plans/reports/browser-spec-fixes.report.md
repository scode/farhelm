## What this was about

Three browser tests had gaps that could hide a regression or make one failure look like many. The sign-in token rotation
test revoked the credential used by later tests before it had saved a replacement. Its own assertion failure or timeout
could therefore leave the rest of the run unable to sign in. The sidebar's long-folder test used a short folder that
never overflowed, so it checked text direction without checking the promised leading ellipsis or preservation of the
path's end. The optional real-Claude spawn test could stop cleanup early, leaving test scratch and shared permissions
behind.

Triage approved these as small test fixes, with a complexity gate if execution needed a refactor or product decision.
All three stayed within the agreed test files and bookkeeping.

## Things you should know

Rotation recovery now runs in an after-hook with its own timeout budget, using the current token and a fresh request
client. It saves only the credential in the existing file shape and permissions. Recovery failures are reported beside
the original test failure. The successful browser sign-in path is unchanged.

The folder test now starts a session in a real long ASCII path and establishes actual overflow before checking
directions, which end lies outside the row, and the computed ellipsis styling. This observes the conditions that make
the browser draw the ellipsis; it does not inspect pixels.

Real-Claude cleanup captures the workspace command's stderr and quietly tolerates only a missing workspace. Scratch
removal and both shared permission restores are attempted even after an earlier cleanup failure. Unexpected
workspace-forget errors are reported before a later restore could replace them, and a failed test body's original error
remains visible.

The actual real-Claude session/spawn leg was not executed because it requires vendor credentials and paid turns. Its
cleanup source passed independent review, the edited spec loaded in both engines, and a separate subprocess probe
checked the workspace diagnostic. That probe used jj 0.46.0: a missing workspace exits successfully with a warning; a
command outside a repository fails and does not match the missing-workspace exception.

## Open questions and possible follow-ups

None of the outcomes tripped its complexity gate. No product behavior or shared browser harness was changed. Live
vendor-session cleanup remains unverified at runtime.

## The PRs

- [Draft PR #1913](https://github.com/scode/farhelm/pull/1913/changes) — browser recovery and clipping test fixes, all
  three outcomes in one change.

## Checks run, reused and skipped

The application was built with `cargo build` and `dx build --package farhelm-ui --platform web --release`; browser
dependencies and Chromium/WebKit were prepared. Node 26.11.1, Playwright 1.64.0, and the repository's pinned tmux were
used. An incompatible CLI earlier on PATH refused the browser build before compilation; selecting the installed Dioxus
CLI completed it.

All browser runs used the recorder from `e2e/`, one worker, zero retries, both engines:

```sh
python3 ../scripts/record-test-run.py --runner playwright --kind development \
  --tmux required --timeout 300 --selection '<named experiment>' \
  --concurrency 'one browser worker; retries 0' -- \
  npx playwright test 'auth\.spec\.ts' 'sidebar\.spec\.ts' 'spawn\.spec\.ts' \
  -g 'rotation logs out|a long cwd shows its tail|a real Claude creates'
```

The pinned tmux was first on PATH. The normal selection passed four cases, with the two real-Claude cases explicitly
skipped (run `aaef3351-b2c1-4952-a94d-3c7cc72a8b3f`). Temporary experiment runs used `--kind repetition` and the
narrower selections below. These were deliberate failures, retained as evidence and removed from the final source.

- Post-rotation throw, selecting auth/sidebar with `-g 'rotation logs out|a long cwd shows its tail'`: both rotation
  tests reported the injected error, and both later folder tests passed (`ad7c12b2-e53e-4ce7-b101-a2d94a4b791d`).
- Post-rotation hang, the same auth/sidebar selection: both rotation tests timed out, and both later folder tests passed
  (`80d19fb0-d14e-4d2b-a208-23eb2ef3cc26`).
- Failed recovery exchange, auth only: Chromium retained the injected original failure beside the secondary 401 recovery
  diagnostic; WebKit then failed sign-in because the experiment deliberately left the shared credential revoked
  (`7da461a4-927c-4544-b59d-4736d3c60890`).
- Missing ellipsis, sidebar only: both engines failed the computed ellipsis comparison
  (`aa84b4fd-18f5-44e7-979f-576e7f181ad0`).
- Tail clipping, sidebar only: the initial block-span mutation failed the overflow premise
  (`393b0aee-b7ec-47f9-9805-11e013f8b0ab`). A refined 40-pixel shift retained overflow, logical text and both
  directions, then failed the new tail-alignment assertion in both engines (`40ae05b6-2dd4-48f3-99bf-2ff0ecb081ec`).

The real-jj Node subprocess probe passed for missing-workspace and non-repository cases. The isolated
`python -B scripts/check-test-sleeps.py` passed: 287 delays inspected, zero missing rationale.
`dprint check TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md` passed. A separate documentation pass covered all three
edited specs.

No Rust runtime tests, Clippy, desktop, installer, website, or full browser battery were run: the diff changes only
three browser specs and outcome bookkeeping, and the focused runs exercise the affected browser behavior. Existing
builds were absent, so they were rebuilt; the prepared builds were then reused across fault experiments. No hosted CI,
deployment, release, ready transition, or merge was performed by the executor.

Main advanced during PR bookkeeping with the notification/read-mark fixes (#1907) and launcher retry, template-error and
frozen-terminal copying fixes (#1908). Every intervening diff was inspected before a clean rebase onto `e64574a9`. The
new explicit mark refresh remains safe when authentication unmounts the list; it does not change the folder label's
layout. Launcher choices and frozen reconnect clipboard handling are separate from these tests. No spec, shared helper
or browser configuration changed, and no conflict needed correction. The normal browser evidence was reused from the
dirty working tree on base `b3d61025` (source fingerprint
`9e8d3ef858c4b558f57aad87d69a21593a7ab3cb7b0ccdf30950d05b4922390f`): the two exercised specs were restored byte-for-byte
after experiments, and the later change to the optional vendor spec was documentation only. The inspected rebase leaves
those checks' coverage applicable; no new runtime checks were needed.

## The review gate's outcome

A fresh-context native gpt-6.1-sol/high source reader reviewed correctness, design, idiomaticity and the full
test-authoring checklist. It found that a later restore could hide an unexpected workspace-forget error. The diagnostic
is now recorded at that error's own boundary; the same reader confirmed closure with no remaining findings. A fresh
wording cold read also passed after clarification. The source review did not execute runtime checks; the executor ran
and inspected the evidence above.

Implementation and validation stayed local as required by the plan. Only the required source, wording and native
resume/report readers were delegated. The wording readers requested gpt-6.1-sol/medium; the source reader requested
gpt-6.1-sol/high; the plans readers inherit the executor's model. Native tools did not expose actual model attribution
or token usage; requested routes are not accounting proof. Private orchestration records remain in the plan's working
log and the session evidence directory identified there; late records for the second wording reader and source
continuation are explicitly marked as a recording gap.
