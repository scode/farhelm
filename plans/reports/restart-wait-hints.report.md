### What this was about

While Farhelm has not yet captured a session's conversation, Restart and Restart with are greyed out. Their hover text
said that no conversation Farhelm can resume was captured, so Replace starts the session over. That read as final and in
Farhelm's own terms, although for most agents it is a normal, temporary wait: Codex, for instance, reports its
conversation only when the first prompt is submitted. Your TODO entry "Explain unavailable Restart in the user's terms"
asked for per-agent wording that names the agent. In planning you chose to state the expected behavior first with a
request for feedback in parentheses (since Farhelm cannot tell a report still to come from one that never will), to give
an ended session final wording instead, and to carry the timing into the supervisor's refusal and the agent instructions
without the feedback request.

That is what landed:

- **In the app, while the session runs**, the Restart control's hover text and screen-reader description now read, for
  Codex: "for Codex sessions, Restart becomes available once you submit your first prompt; until then, Replace starts
  the session over. (If Restart doesn't become available, please send feedback from the help (?) menu.)" Each agent gets
  its own moment, meaning the point at which it reports the conversation: Claude a few seconds after it starts (its
  startup hook), Goose as soon as it starts, Pi and OMP once their conversation file is saved, which happens with the
  agent's first reply, Grok after the first prompt and only if Farhelm's Grok hooks are installed (pointing at the Grok
  page of the documentation). An agent the app does not recognize gets the same shape without naming one.
- **Once the session has ended** (exited, failed to start, or interrupted by a reboot): "Farhelm never captured this
  Codex session's conversation, so Restart can't resume it; Replace starts the session over. (If you expected Restart
  here, please send feedback from the help (?) menu.)" The interrupted card says the same, including for sessions from
  before launch kinds, whose card offers Replace with.
- **Restart with's** greyed-out reason gives the same fact in a short form ("for Codex sessions, Restart with becomes
  available once you submit your first prompt", or "Farhelm never captured this Codex session's conversation"), with the
  same feedback request.
- **The supervisor's refusal** of such a restart, which agents read through `farhelm agent restart`, names the agent and
  says when Restart normally becomes available, or for an ended session that its conversation was never captured, with
  no feedback request. **The agent instructions** list each agent's moment, generated from the same facts.
- SPEC.md's sentence on greyed-out Restart says all this; the TODO entry is removed.

### Things you should know

- **A refused restart of a never-prompted Codex session used to give the wrong reason.** It answered with "its legacy
  identity is unattributed, or its exact record is unavailable", a check written for Codex sessions that captured an
  identity Farhelm cannot verify. It now applies only to those; a session that has captured nothing gets the new
  explanation.
- **The help menu is named "help (?) menu"** rather than "? menu" as the plan's example had it: the button's visible
  face is a question mark but screen readers announce it as "help", so the text names both.
- **The wording addressed to agents differs slightly from the app's**: the app says "once you submit your first prompt",
  while the refusal and instructions say "once the user submits the first prompt", because an agent reading "you" would
  take it to mean itself. Both come from one place, so the app, the refusal and the instructions cannot disagree about
  an agent's moment.
- **"Running" for this purpose** is every status except exited, error and interrupted, so a session whose status is
  unknown (for example, on a host that is not answering) gets the waiting wording, with the feedback request as the
  hedge.
- **No change** to when Restart is offered, to the wire protocol, or to how conversations are captured.
- The table of moments was checked against each agent's reporter before writing it; no corrections were needed.

### Open questions and possible follow-ups

- None needing a decision. In-app feedback reaches you only once its endpoint is set up (your separate TODO entry);
  until then the feedback request leads to the dialog's ordinary failure message, as you decided in planning.

### PRs

- #1618 `feat: say when Restart becomes available for each agent` — the whole change, with its changelog fragment.

### Checks

Run ids are the recorder's retained test runs (`scripts/record-test-run.py`):

- Rust tests covering the change in the protocol crate, the app's session view, the supervisor's restart path and the
  agent CLI's instructions (215 tests, run 28cc8a0f, after rebasing onto the latest main).
- The browser spec for Restart and the interrupted card, on Chromium and WebKit (30 passed, run 1622ac91), on the commit
  before the final rebase. Reused: the rebase brought in app-bar and settings changes from other plans, not the session
  view's restart controls this spec drives.
- `cargo fmt --check`, `cargo clippy --all-targets -D warnings`, `cargo clippy -p farhelm --bins`, `dprint check`, the
  changelog format check and the test-sleep check: clean.
- Skipped: the full Rust battery and other browser specs (the change is text in a few functions with direct tests),
  desktop, installer and release-only checks (unaffected).

### Review gate

Two reviewers (a Claude and a GPT model) on the PR, then a second round on the fixes. Fixed: the pre-launch-kinds
interrupted card skipped the new explanation; the supervisor's refusal gave the waiting timing for an ended session;
Restart with's reason was the full tooltip rather than a short form; the "? menu" naming; the old Codex refusal still
running for other kinds of unavailable Restart (a Codex command launch without a resume command got the identity message
instead of "no resume command"); and a few small documentation and test gaps. Nothing declined. The second round found
nothing.
