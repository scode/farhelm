# The Cmd+W double-tap test claims a second press can never reach the session, but pins only the in-flight window

Reviewed commit: a624f5f80fbab8188716aa337403d96d2da80b04

## TLDR

A test of the Mac Cmd+W shortcut says a double tap on a shell tab must never reach the session, but it checks only the
moment the tab's close is still in flight. With Skip session confirmations on, a second Cmd+W that arrives after the
only (or first) shell tab has closed deletes the session without a question, which SPEC.md as written accepts; the test
text promises more than the product does, and whether the product rule should be widened is a triage decision.

## Details

Narrowed at recording: the behavior below is accepted by SPEC.md (see Triage notes); what remains queued is the test
docstring at `crates/gui/farhelm-gui/src/terminal/set/tests.rs:3282` overstating the guarantee, plus the product
question. The reviewer's original analysis follows.

`TerminalSet::close_shown` (crates/gui/farhelm-gui/src/terminal/set.rs:1383-1406 at the tip) returns `true` (so
`MainView::delete_or_close_tab`, crates/gui/farhelm-gui/src/main_view.rs, does not fall back to Delete) only while
`chosen` is still the closing tab: `confirmed_closed.contains(&id) || closing(&key, &id, cx)`. But `close_tab`'s settle
callback (set.rs ~1430-1445) moves `chosen` to `previous_surviving_tab` the moment the DELETE succeeds, which is `None`
(the main terminal) when the closed tab was Terminal 1 or the only tab (SPEC.md, "Session view": "The main terminal
takes over when there is no earlier tab"). From then on a Cmd+W press reaches `header_action(Delete)`. While the pushed
row still lists the tab, `delete::asks_despite_skipping` (crates/gui/farhelm-gui/src/session_list/delete.rs:268-279)
still asks; once the row stops listing it, `delete::asks` with skipping on sends the Delete at once. So the only window
the in-flight guard closes is the DELETE round trip; a double tap slower than the close round trip plus the row push
deletes the session. The guard's test, `a_second_cmd_w_while_the_tabs_close_is_in_flight_does_nothing` (set/tests.rs),
leaves the DELETE unanswered, so it never exercises the post-settle path, yet its docstring says "A double tap of a
Mac's Cmd+W on a shell tab must not reach the session" and the confirmed_closed check in close_shown can never fire for
the chosen tab because settle moves `chosen` off it in the same update. The behavior also follows the spec literally
(main terminal shown => Cmd+W deletes), so this is a design-level hazard the spec accepts rather than a contract
violation. Verify by reading close_tab's settle callback and close_shown together. A fix could keep Cmd+W from falling
through to Delete for a short time after a Cmd+W closed a tab (e.g. keep a per-set "closed by shortcut, falling back"
marker until the next non-close key or for a debounce interval), or require the Delete question whenever the previous
Cmd+W in this session closed a tab; either needs a SPEC.md change.

### Triage notes

- Recorded bucket: `other`. Narrowed by the independent drop check. The behavior the reviewer described (with Skip
  session confirmations on, a second Cmd+W that arrives after the only or first shell tab's close has landed deletes the
  session without a question) is accepted by SPEC.md, "Session view" ("With the main terminal shown, ... Cmd+W deletes
  the session instead; with a shell tab shown whose close is already in flight, Cmd+W does nothing") and by the Skip
  session confirmations paragraph, whose open-tab exception holds only "while that session has an open terminal tab";
  that part was dropped (consultant AGREE, checked SPEC.md:2574-2576 and 2378-2385, set.rs:1430-1440,
  delete.rs:278-280). Not covered, and what this item keeps: the test docstring at
  crates/gui/farhelm-gui/src/terminal/set/tests.rs:3282 claims "A double tap of a Mac's Cmd+W on a shell tab must not
  reach the session", but the test pins only the in-flight window. The consultant noted SPEC.md:2385's stated purpose
  ("so that a keystroke meant for a tab cannot end the session") pulls against the literal rule; whether to widen the
  rule is a product decision for triage.
- Suggested bucket (reviewer): highest if the double-tap route is accepted as real (a session delete kills the agent and
  its processes, which the bucket names), otherwise high; uncertainty: the route needs the second press to land after
  the helm has answered the tab close and pushed the row, a round trip of local supervisor work that may or may not
  exceed a typical double-tap interval. The user also opted out of confirmations, and SPEC.md accepts the outcome.
- Confidence: likely; the unverified premise is that a tab close's DELETE plus row push settles within a human
  double-tap interval in practice.
- possible_cover: none known; g10 (delete with open tabs) owns `asks_despite_skipping` and may have related records.
- proposed_drop: SPEC.md, "Session view": "With the main terminal shown, ... `Cmd+W` deletes the session instead (see
  Desktop window chrome); with a shell tab shown whose close is already in flight, `Cmd+W` does nothing." and SPEC.md
  "Skip session confirmations" paragraph: "It does not skip a single session's Delete (row, header or shortcut) while
  that session has an open terminal tab either, so that a keystroke meant for a tab cannot end the session and every
  tab". The spec protects only the in-flight window and only while a tab is listed; the commissioner decides whether the
  gap is worth queuing. At minimum the test docstring overstates the guarantee.

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g09/f1 (finding), group g09 "Open and close terminal tabs with Cmd+T and Cmd+W", findings log line 1.
