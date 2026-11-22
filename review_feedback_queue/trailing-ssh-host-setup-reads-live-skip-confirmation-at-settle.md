# SSH host setup and setup Re-run read the live skip-confirmation preference when the probe settles

Reviewed commit: dfdef7da1653956bb4b48a74281f72d990863bba

## TLDR

When you add an SSH host or re-run a failed SSH setup, Farhelm checks the "Set up new hosts without asking" setting only
after the remote probe finishes instead of when you clicked Add or Re-run. If that setting turns on while the probe is
still running, Farhelm starts installing on the remote host immediately without showing the setup plan for confirmation.

## Details

**high.** Source inspection. Main reference `crates/gui/farhelm-gui/src/hosts/setup.rs:796`.

In crates/gui/farhelm-gui/src/hosts/setup.rs:796 (inside setup::rerun's probe completion callback; file touched by
commit dfdef7da1653956bb4b48a74281f72d990863bba, pre-existing behavior) and crates/gui/farhelm-gui/src/hosts/add.rs:1287
(inside AddHostDialog::settled), the GUI calls setup::asks_first(cx) when the asynchronous POST /api/hosts/probe reply
arrives rather than capturing the skip_host_setup_confirmation preference from the frame when the user clicked Add or
Re-run. If skip_host_setup_confirmation was false when the user initiated the probe (expecting to review the plan steps
before anything is installed) and becomes true during the probe (e.g. from another client or a concurrent
permanent-answer save), the completion immediately submits POST /api/hosts/provision via setup::submit(offer, ...). This
violates SPEC_impl.md, "Questions: the prompt" (lines 993-997): a handler must carry the drawn skip preference and use
that snapshot rather than recomputing it at send time. Note: agent-requested host adds are unaffected because
add.rs:1287 requires !self.from_agent, and Tensorlake sandbox reruns intentionally fetch GET /api/tensorlake when the
plan arrives (setup.rs:857-881). Fix direction: capture asks_first at the Add / Re-run control's ActionFrame / click
handler and carry that boolean through the probe callback into settled() and rerun().

Reviewer metadata (g30-gui-refactors-approvals-hosts-prefs/f2): suggested bucket: high (could not be highest:
provisioning installs Farhelm's supervisor binary and user service on a remote SSH host the user explicitly chose to Add
or Re-run without destroying user data, though it skips the installation-plan confirmation the user expected to review);
confidence: confirmed (source and contract analysis); possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as
`g30-gui-refactors-approvals-hosts-prefs/f2`.
