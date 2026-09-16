# Island cap: the never-connected first mount

Historical record written 2026-09-16. This is not a diagnosis of why one handshake stalled — that was still a
hypothesis when writing stopped — it is the evidence trail that replaced the "established socket died" reading with the
first-mount chain, plus the fix fork as it stood. Like every lore entry it is frozen: later work may supersede it, but
this file stays as written.

Two WebKit tests in `e2e/tests/terminal-tabs.spec.ts` share this history: `stalling one tab's writes pauses only that
tab; the agent and a sibling stay live`, and `a tab list past the island cap is listed in full but only partly
attached`. The island-cap test is the one that got hunted to a mechanism; the stall test is the sibling that pointed
the same way and then went quiet.

## How it started

Both shapes come from browser run `7fd44a19-ce3f-42fb-a3df-410da327634a`, on frozen composer candidate
`f0aa71e488d1cb2f55099865b85c23025c106edf` (full Chromium/WebKit suite, one worker, zero retries): the stall test never
established a HIGH_WATER pause within its observation window, and the island-cap test had the expected path and
mounted/revealed state but a closed socket at readiness.

The island-cap shape reproduced 3/20 recorded WebKit repetitions on 2026-09-12 (batch
`396d6272-a07d-4bd4-b9f1-6409bc916cc9`, timelines attached in the run): the agent terminal's established socket errored
and closed about two seconds into the 32-phantom attach-refusal storm, with no helm log line for it; every
reconnect-ladder attempt after that was refused within milliseconds for the rest of the test, and one past-cap phantom's
attach retried on a ladder of its own for 20s. The helm logged only the phantom refusals ("has no terminal tab"); the
supervisor's log was silent; what killed the agent attachment was not established. That was the "established socket
died" reading, and it stood for four days.

The stall test's zero-pause shape also reproduced once in ten recorded repetitions (batch
`e5f7f17f-7f24-4118-8559-62a1ea5df624`, then 24 consecutive passes): one tab's socket closed about 1.2 seconds after the
flood started — before any HIGH_WATER crossing, with no helm log line — the pause poll then waited its full sixty
seconds over a dead socket, and at the supervisor's stall interval the session's other two sockets closed before
reconnect ladders that were refused instantly. Same early-silent-close signature as the island-cap reproduction, and
consistent with the helm's outbound side winning the race before the browser could pause. The closing half's own receipt
(detach reason, queue depth) still needed a supervisor-side log from a reproduction — that gap is what the September 16
hunt closed by preserving the stack's supervisor logs past teardown.

## The hunt that replaced the reading

A 20-repetition WebKit island-cap hunt on 2026-09-16 (run `4f6d89c8-2911-4672-877d-751b517a6be0`, stopped after 2
failures in 15 runs) reproduced with supervisor logs preserved, and both failures share one signature that supersedes
the "established socket died" reading: the agent island shows M5's "never finished connecting" banner — the 5s replay
idle timer expired with the socket still CONNECTING, bannered, closed it, and (first-mount rule) never retried. The 20s
is the readiness budget polling that dead island, not a second mechanism. Each trace carries exactly one agent network
entry (101 received) and one console "closed before established" error; both traces plus the tailed supervisor log are
preserved unpacked under `analyst-supplement/` in the run record — Playwright wiped the checkout's `e2e/test-results`
on a later run, so the run record is the only copy. No retained helm line identifies the agent socket, and the
supervisor logged only session create/teardown — though the helm's generic "no such session" lines alongside the
phantom refusals cannot all be attributed elsewhere either.

So the established chain is: handshake accepted, `open` never fired, bannered and closed at 5s, never retried. Why the
browser held that one handshake while 32 phantom upgrades churned beside it is a hypothesis — burst churn starving one
completion, possibly the same family as the rotation unanswered reads — not a measured cause.

## The M5 model, and why the fix is a decision

The island code already anticipates this shape and deliberately does not recover it. `terminal.js` says it plainly
("only a socket that worked and then stopped is something to recover"): a first mount against a helm that is not there
keeps the banner rather than silently retrying behind it. That model's world has two states — helm there, helm not
there — and this failure is a third: helm there, but one handshake stalled. Retrying would contradict the comment's
letter, and the comment was written deliberately. Hence the fork:

- Retry a never-connected first mount on the ladder. A product change against the M5 comment's letter: a genuinely
  unreachable helm would now retry instead of banner-and-stop, so the new model needs maintainer judgment.
- Accept the burst as test-only pathology the design need not survive. Cheapest — but staggering the mounts to avoid
  the burst weakens the oversized-at-once fixture, and the test would stop exercising the 33-simultaneous-attach shape
  that found this.
- Keep digging browser-internally for why one handshake stalls amid churn. No behavior change, unknown payoff.

The stall test was not re-hunted; its 24 consecutive passes stand. If it recurs, it re-enters with its own evidence —
the island-cap chain does not explain a socket that closed 1.2s after the flood started.
