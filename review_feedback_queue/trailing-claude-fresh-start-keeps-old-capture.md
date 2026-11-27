# A Claude Restart that started fresh keeps the stale capture, so if the new run never reports, every later Restart silently starts over and abandons the conversation the user had in between

Reviewed commit: 4f0692f0387d6d35039f2e2125d2924569953d14

## TLDR

When Restart starts Claude fresh (its reported transcript was missing), Farhelm keeps pointing the session at the old,
never-saved conversation. If the new Claude cannot report its own conversation, the user then works in a conversation
Farhelm never learns about. The next Restart (including the interrupted-session card's Restart after a reboot) checks
the old missing file again and silently starts yet another blank Claude, below the previous run's scrollback, so it
looks like a continuation. SPEC.md says a run whose report never arrived has Restart unavailable with a reason, and that
a Restart that silently loses the conversation is a bug. Before this change the same Restart failed visibly
(`claude --resume` of an id that never existed).

## Details

In `restart_session_named` (core.rs ~12857-12876 at the tip) the fresh choice sets only `(argv, phase)`. `clear_capture`
(core.rs ~12910-12930) is computed only for named starts, and `begin_relaunch_named` keeps
`snapshot.captured_conversation` (the old `claude:` locator with the missing transcript). Nothing on the Start phase
withdraws or marks the capture. The restart offer is not gated on the hook switch (no `agent_hooks` use in the offer
path; `unavailable_reason` takes kind, launch kind, ended, persistent hook setup), so Restart stays offered. The named
path already handles the analogous problem by clearing the capture when the new launch has no ordinary reporter
(`AgentHooks::silences`, agent_kind/mod.rs:2934; `custom_reporting() == Some(false)`; `reporter_failed`), and the fresh
path applies none of these. SPEC_impl.md, "Restart when Claude saved nothing", says: "The old capture is kept: the next
Restart checks the same missing file and starts fresh again, so the stale ID is harmless, while clearing it would leave
Restart unavailable whenever the new run cannot report." That premise holds only when the new run reported or saved
nothing. When the new run has real but unreported turns, the stale capture turns SPEC.md's outcome ("A launch whose
harness cannot report, or whose report never arrived, has no captured identity and cannot be restarted: Restart says
why") into a silent fresh start.

Established route: Claude session captured `claude:{X, file}` before any prompt; the supervisor is later started with
FARHELM_AGENT_HOOKS excluding claude (sessions survive a supervisor restart, and `silences(Claude)` is then true);
Restart finds the file missing and takes the Start phase with capture X kept and no hook injected; the user works in
conversation Y, which is never reported; a later Restart finds X still missing and starts fresh again, so Y is not
resumed and nothing says so. Other plausible routes are unverified: Claude's own `disableAllHooks` setting suppressing a
command-line `--settings` hook, a command launch's Restart with adding `--settings` (hook_refusal), and a runtime hook
failure. Fix options: on a Start-phase relaunch, clear the capture (or mark it consumed) when the launch is known not to
report, using the same predicate as the named path; or record that a fresh start consumed the capture so a later Restart
with it unreplaced is refused with the not-captured reason. Then reconcile the SPEC_impl sentence with SPEC.md.

Reviewer revision notes: Revision (turn 2): re-checked against source; the route is narrowed to the one established from
code, and the other routes are labelled unverified. The rest is unchanged.

### Triage notes

- Recorded bucket: `highest`. The reviewer suggested high. Recorded as highest because the bucket is uncertain: the
  established route silently replaces a valid Resume of a real conversation, which FILTER.md's session filter lists
  among losses of user-owned work. The conversation file stays on disk and Restart with can reopen it if the user finds
  its id; triage can lower it on that basis.
- Suggested bucket (reviewer): high. It silently drops Restart's continuation of a real conversation in a plausible,
  non-default configuration. The conversation file stays on disk and Restart with can reopen it if the user finds its
  id, which is why it is not highest. If triage treats an unannounced, unresumable conversation as lost user work, it is
  highest.
- Confidence: confirmed from source for capture retention and the FARHELM_AGENT_HOOKS route; likely for the other
  routes, whose unverified premise is that Claude, under those settings, fails to deliver the command-line hook.
- possible_cover: none known.
- proposed_drop: none. SPEC_impl.md "Restart when Claude saved nothing" keeps the capture deliberately, but on the
  stated premise that the stale id is harmless, which this route contradicts, and against SPEC.md's rule for runs whose
  report never arrived.

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g16/f1 (finding), group g16 "Claude restart fresh when no conversation was saved: spec and supervisor", findings log
  line 6.
