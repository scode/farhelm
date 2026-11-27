# A Delete whose prompt, or skipped click, showed a running agent and no terminal tabs still ends a tab opened after that frame, because no guard says "no tabs"

Reviewed commit: b2d540a8dacfb94140e5eb0927d78e447ce46111

## TLDR

When you delete a session whose agent is running and whose row shows no shell tabs, Farhelm authorizes ending everything
in it, not just what you were shown. If a shell tab appears after the row was drawn, the Delete ends that tab too and
never says so. This happens whether the Delete asked (the question named only the agent) or ran on the first click with
Skip session confirmations on. The spec says such a Delete should be refused and asked again.

## Details

- Code: delete::guard (delete.rs:86-94) maps NothingAlive only to an ended agent with no tabs, AgentEnded to an ended
  agent with tabs, and every agent that has not ended (or whose status is unknown) to DeleteGuard::Unconditional,
  whatever the row showed about tabs. The protocol has no variant to express "agent may be stopped, refuse if any tab is
  open": farhelm-proto/src/lib.rs:2962-2970 defines only Unconditional, AgentEnded and NothingAlive. delete::command
  then adds no flag for it (delete.rs:335-343), and send_delete (delete.rs:360) sends that request. The question freezes
  that guard at the press (delete.rs:282-300) and does not reword while it is open (ask_about_row, menu.rs:642-676, uses
  plain ask/ask_anchored, not ask_rewordable). The same guard is reused by single Replace (replace.rs:137, 261), bulk
  Delete (bulk.rs:318, 323) and bulk Restart/Replace's Replace (bulk.rs:453).
- Spec: SPEC.md:2392-2410 ("Confirmed 2026-10-01"): "a destructive confirmation authorizes only what the prompt the user
  answered said would happen ... never a state the user was not shown. When the session setting skips a prompt, the
  click authorizes only the state the screen showed, under the same guards. The request carries the matching
  precondition, and the supervisor refuses, at the moment it acts, when the session has more alive than that." The only
  accepted race is narrower: "Accepted: a terminal tab opened between a prompt that named open tabs (alone, or together
  with stopping the agent) and the click is closed with the others". A prompt or click that showed no tabs falls outside
  it. Stop does have this precondition ("a Stop whose click showed no tabs is refused if any tab is open", and
  SPEC.md:2673's bulk "separate no-tabs precondition"); Delete and Replace do not. One counter-reading: the spec's list
  of Delete/Replace refusals names only the nothing-alive and tabs-only prompts, and someone could read that list as
  exhaustive. But the explicit "Accepted" carve-out would then be pointless for this case, so I read it as a gap, not an
  acceptance.
- Route (source): only the GUI opens tabs (helm POST /api/sessions/{id}/tabs, lib.rs:922-924; no CLI command found in
  crates/farhelm/src), and a helm takes one UI at a time (SPEC.md, "One GUI at a time"). The reachable route is
  therefore the GUI's own tab request racing the Delete. For example, with Skip on, a Mac user presses Cmd+T and then
  Cmd+W before the new tab is listed and shown: Cmd+W finds no shell tab shown, so it reaches the header Delete
  (main_view.rs:190-203), and the drawn row lists no tab, so asks_despite_skipping is false and the Delete goes with
  Unconditional. With confirmations on, a New terminal request still in flight when the question opens lands while the
  question is up, and Confirm sends Unconditional. In both cases the tab ended is one the user had just asked for.
  Neither sequence was executed.
- Fix: add a DeleteGuard variant (e.g. a no-tabs guard that refuses while any tab is open but stops the agent) with its
  query flag in delete::command and the supervisor's check (farhelm-supervisor core.rs:13705-13780 handles the existing
  ones), and send it from guard() for a row whose agent has not ended and that lists no tab. That is a protocol change.
  Then update the NOTE at delete.rs:273-276, which currently presents the behavior as intended. Verify by reading
  guard() against the SPEC.md refusal list and by a supervisor test that a no-tabs Delete is refused once a tab is open.

Reviewer revision notes: Revision (second event: corrects the function named for delete.rs:335-343). Reclassified from
candidate to finding after the independent drop check. My earlier basis ("judged from the row as drawn") settles only
whether Delete asks, not what the request authorizes. The commissioner's lead 2 (the same gap with confirmations on) is
the same defect and is folded in here, so it has no separate record.

### Triage notes

- Recorded bucket: `highest`. The reviewer first set this aside as spec-accepted; the independent drop check disagreed
  (SPEC.md:2397-2399 lets a skipped or confirmed Delete end only what the screen showed, and SPEC.md:2409-2410 accepts
  an extra tab only after a prompt that named open tabs), and the reviewer then recorded it, folding in the consultant's
  lead that the same gap applies with confirmations on. The reviewer suggested other, with high arguable, because the
  only shipped route it found ends a shell tab the same user opened moments earlier. Recorded as highest because the
  bucket is uncertain and the consequence is harm beyond what a destructive action's confirmation showed, which
  review_feedback_queue/AGENTS.md, "Priority buckets", keeps in highest; triage can lower it if no route to a tab
  holding real work exists. The behavior predates the range; 52fec2470f documented it in a NOTE (delete.rs:273-276).
- Suggested bucket (reviewer): other, with high arguable. Its class is process loss beyond what a destructive
  confirmation said, which the bucket rules keep in highest whatever the staleness. But the only route I could establish
  in shipped code ends a shell tab the same user requested moments earlier, which holds no work yet and is recreated in
  a moment. It would move to highest if a shipped route lets a tab with real work appear unseen between the drawn frame
  and the press: a second UI, a CLI or agent command that opens tabs, or a listing that lags a tab's creation for long
  enough for it to be used. I found none.
- Confidence: confirmed for the code behavior (source: delete.rs:86-94, proto lib.rs:2962-2970). Likely for the spec
  violation; the unverified premise is that the spec's general "more alive than that" rule, not its enumerated list,
  governs the running-agent, no-tabs case.
- possible_cover: none known. 52fec2470f's NOTE at delete.rs:273-276 records the behavior as a known consequence, which
  may mean the maintainer considered it.
- proposed_drop: none

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g10/c1 (finding), group g10 "Session list menus: delete with open tabs, marked menu focus", findings log line 10.
