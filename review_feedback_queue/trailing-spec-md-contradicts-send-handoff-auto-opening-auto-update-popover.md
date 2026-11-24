# SPEC.md:5917-5919 contradicts SPEC.md:3741-3742 and sign_in.rs on whether Send handoff can auto-open an automatic Update Setup popover

Reviewed commit: d26b59f312b600ebbfb1ce7029d1ae8edfab045a

## TLDR

The product specification contradicts itself on when an automatic host update's SSH sign-in popover can open on its own:
one section says it opens only when you press **Answer** (or run a manual action on that host), while the keyboard-focus
section and the desktop app allow sending an answer to any other host's SSH question to immediately open and focus an
already-waiting automatic Update Setup question.

## Details

**other.** Source inspection. Main reference `SPEC.md:5917-5919`.

- Commit `d26b59f312b600ebbfb1ce7029d1ae8edfab045a` changed `SPEC.md:5917-5919` (under "Supported user environments")
  to: "An automatic Update's Setup question waits in the attention row, and its popover opens only when the user presses
  **Answer**. A manual action that needs the same connection can open the question already waiting for that host."
- That exhaustive "only when the user presses **Answer**" wording omits and conflicts with `SPEC.md:3741-3742` (under
  "Keyboard focus" -> "Who may move focus"): "And after the user sends an answer while the field still has focus, the
  field may move to the same sign-in's next question or to another question that was already waiting."
- In shipped desktop code (`crates/gui/farhelm-gui/src/sign_in.rs:129-134,272,322-338`), sending an answer from any SSH
  sign-in popover snapshots every other currently open `SshQuestion` (without distinguishing automatic-update Setup
  questions, which carry no flag separating them from manual Setup questions) into `Followup::already_waiting`. When the
  answered question closes, `SignIn::consider` opens and focuses the waiting automatic-Update Setup popover without the
  user pressing **Answer** or initiating a manual action on that host.
- How to verify: inspect `SPEC.md:3741-3742`, `SPEC.md:5917-5919`, and
  `crates/gui/farhelm-gui/src/sign_in.rs:129-134,322-338` (or open an automatic Update Setup question on host A
  alongside a Sessions question on host B, answer host B's question, and observe host A's Setup popover opening
  automatically).
- Fix direction: either qualify `SPEC.md:5917-5918` to acknowledge the existing Send-to-already-waiting-question focus
  handoff in `SPEC.md:3741-3742`, or (if automatic Update Setup questions are truly meant to never open via Send
  handoff) mark automatic-only Setup questions on `SshQuestion` and exclude them from `Followup::already_waiting` in
  `sign_in.rs:322-333`.

Reviewer metadata (g39_host_auto_update_2fa_impl/l4): suggested bucket: other (documentation/spec contradiction and
minor focus-handoff ambiguity; no security, data-loss, or material UX degradation).; confidence: confirmed; possible
cover: none.

Additional source record at `1b3fe49af86979aebc2719965682a17f8c6366ac`,
`crates/gui/farhelm-gui/src/sign_in/tests.rs:451` (g40_host_auto_update_2fa_reconnect_tests/l2):

Reason: spec-accepted (SPEC.md, "Who may move focus", lines 3738-3744: "Questions from later attempts, including retries
for hosts that were unreachable at startup, wait in the sidebar, with two exceptions. A sign-in the user just started
may open its question. And after the user sends an answer while the field still has focus, the field may move to the
same sign-in's next question or to another question that was already waiting."). In
crates/gui/farhelm-gui/src/sign_in.rs:134-139 and 322-338, Followup::already_waiting intentionally captures all pending
SshQuestions at Send time and hands focus to one after a 204 reply, whereas a Setup question arriving after Send on a
different attempt does not open automatically (verified by
automatic_setup_on_the_same_host_does_not_inherit_sessions_focus in
crates/gui/farhelm-gui/src/sign_in/tests.rs:451-491). Note for g39_host_auto_update_2fa_impl (which owns SPEC.md):
SPEC.md line 5917-5918 ("An automatic Update's Setup question waits in the attention row, and its popover opens only
when the user presses **Answer**. A manual action that needs the same connection can open the question already waiting
for that host.") states "only when the user presses Answer" without mentioning the existing already-waiting-after-Send
handoff in SPEC.md:3741-3742. Could be highest: no

Reviewer metadata (g40_host_auto_update_2fa_reconnect_tests/l2): suggested bucket: other; confidence: confirmed;
possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g39_host_auto_update_2fa_impl/l4`,
`g40_host_auto_update_2fa_reconnect_tests/l2`.
