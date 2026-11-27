# With two different Tensorlake keys typed, saving one and closing now offers Save for the other with no hint it replaces the key just saved

Reviewed commit: ee8ef7294a6638d7905fca7dd27083d0d0bb45c9

## TLDR

Someone types one Tensorlake key on the Tensorlake page and a different one on Credentials, saves the first, and presses
Close. Settings now waits for that save and then asks "Save changes before closing?" naming only "Tensorlake API key in
Credentials", with Save as the starting answer. Pressing Save (or Enter) replaces the key they just saved with the other
one. Before this change the same sequence showed "You entered two different Tensorlake keys" and "If that key is already
saved, discard the draft", a guard written for exactly this case. The question gives no sign that a key was just saved
or that Save would replace it.

## Details

The test `closing_while_a_key_save_is_out_waits_and_asks_about_the_other_draft` (tensorlake_tests.rs:296-299, helper at
301-344) now pins this behavior: after the first key's 200, it asserts the question contains "Unsaved: Tensorlake API
key in Credentials" and does not contain "You entered two different Tensorlake keys." The pre-commit version of the same
test (`closing_with_conflicting_keys_does_not_replace_a_pending_key_save`, see
`git show ee8ef7294a^:crates/gui/farhelm-gui/src/settings/tensorlake_tests.rs`) asserted the conflict message and "If
that key is already saved, discard the draft." SPEC.md "Settings" says "Two different unsaved Tensorlake keys leave
Settings open on Credentials with a message to save the desired key first"; once the first save has landed only one
draft is unsaved, so the new behavior does not contradict the letter of that sentence, but the rule's purpose (Settings
must not pick one of two keys the person typed) is undercut: the close question's Save picks the second key over the one
the person explicitly saved a moment earlier, and the question's line and Save tooltip ("Save all the listed changes,
then close Settings") do not mention replacement. The question opening is the person's fresh decision, so this is a
judgment call rather than a contract break. A fix could keep the conflict check alive for a close that waited on a key
save (treat a draft that differs from a key saved during the wait as a conflict, or add a line saying Save replaces the
key just saved), or the maintainer may accept the new flow and record that in SPEC.md.

Reviewer revision notes: Revision (turn 2): line numbers are at the pinned tip b2d540a8dacfb94140e5eb0927d78e447ce46111
(test at tensorlake_tests.rs:297, helper at 302); unchanged since the reviewed commit. Re-checked against SPEC.md
"Settings": no sentence accepts or rejects this flow, so the bucket and proposed_drop stay. Title: Closing after saving
one of two different typed Tensorlake keys offers to save the other over it, without the former warning.

### Triage notes

- Recorded bucket: `other`.
- Suggested bucket (reviewer): other (the person can re-enter the key they just typed; no lasting loss of user-owned
  work is established).
- Confidence: confirmed for the behavior (the test asserts it); whether it is unwanted is a product judgment.
- possible_cover: none
- proposed_drop: none

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g03/f3 (finding), group g03 "Settings close waits for outstanding work: spec, panes and tests", findings log line 10.
