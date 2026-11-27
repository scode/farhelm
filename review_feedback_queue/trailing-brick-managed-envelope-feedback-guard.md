# The 'Do not select Send feedback now' guard the new comment says stays on every startup checkpoint is absent from the managed instructions that live brick tests deliver

Reviewed commit: d8e9a06b9be99e7b52cf7cb8ae493efbd93629e8

## TLDR

During a brick test the candidate now opens with Welcome inline in the empty main pane, which offers Send feedback. The
harness's plain instructions tell the tester not to send feedback before the P2 feedback checkpoint, but the managed
instructions that real runs use say nothing of the kind on the candidate's first open or after Restart to update, so a
tester exploring the Welcome view has nothing stopping an early, unrecallable live feedback send.

## Details

guest_workflow.py instructions() (around lines 399-412 at the tip) appends 'Do not select Send feedback now or begin P2
early.' to app-start and update-relaunch, with a comment claiming 'The Send feedback guard stays on every startup
checkpoint: the inline Welcome still offers Send feedback, and the live send belongs to P2 alone.' That function only
builds the plain envelope. managed_instructions() returns update_instructions() for update-relaunch
(checkpoint_contract.py, which now says only 'Require the candidate returns usable.') and the contracts dict entry for
'app-start' (guest_workflow.py around line 458: 'Call brick_action launch. Bind the installed Farhelm app through native
CUA and require candidate UI, usable session list and connected candidate local supervisor... No remote setup,
conversations, templates or settings.'), neither of which mentions feedback. The new test
test_every_plain_startup_keeps_the_tester_off_send_feedback (test_guest_workflow.py) documents the gap explicitly ('the
managed one, which live runs use ... has no such guard on the candidate starts') rather than closing it. The removal of
the Welcome card makes the gap more relevant: the candidate's inline Welcome is exactly where Send feedback is now
offered at startup (crates/gui/farhelm-gui/src/welcome.rs: 'The same button receives focus when the user closes Add host
or Send feedback after opening either from Welcome'). Fix: append the same guard to managed app-start and to
update_instructions' relaunch text, correct the comment's scope, and extend the test to the managed envelope.

### Triage notes

- Recorded bucket: `other`.
- Suggested bucket (reviewer): other (maintainer tooling; consequence is a spurious feedback submission and a possibly
  confusing P2 result, not product or user data loss).
- Confidence: confirmed for the absent guard and the comment's overstated scope; likely for the consequence, whose
  unverified premise is that a model tester would select Send feedback without being told to.
- possible_cover: none
- proposed_drop: none

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g33/f2 (finding), group g33 "Maintainer tooling: brick tests and release-notes screenshots", findings log line 3.
