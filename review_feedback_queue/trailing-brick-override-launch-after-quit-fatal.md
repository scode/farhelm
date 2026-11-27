# launch_override treats an app still exiting after the structured override Quit as a fatal error, unlike reopen and announcements

Reviewed commit: 6baa2a6ac5169cc21d24b91d5aefb8897df08662

## TLDR

In a brick-test updater-override pass, the tester now quits the previous release through the harness's own Command-Q
action. If it then calls the managed override launch before macOS has finished terminating the old app, the harness
closes the test action gate and the whole attempt fails, with an error telling it to 'quit the app normally through
native UI' even though it just did. The same race on the reopen and signed-announcements checkpoints is a retryable 'not
ready' response.

## Details

6baa2a6 routes the override Quit through NativeUI (guest_ui.py require_quit_ready / quits_for_update_override, completed
'override_quit'). The Quit is asynchronous: the provider only acknowledges the key press (guest_ui.py require_quit_ready
docstring: 'The provider acknowledges input. The managed launch separately checks for the process to exit'). In
guest_operations.py perform() for launch/launch_override (around lines 751-755 at the tip), when
version()['desktop_running'] is still true the code raises ObservationNotReady only for check in ('p2-announcements',
'reopen'); for update-discovery/update-relaunch it raises RuntimeError('quit the app normally through native UI before
managed launch'). dispatch() (lines 487-497) turns any non-ObservationNotReady exception into self.fail(...), closing
the gate for the rest of the attempt. The new instruction text ('Wait for versions to show that the old app exited, then
call launch_override', checkpoint_contract.py update_instructions) relies on the tester polling first; a tester that
calls launch_override directly after the harness's Quit loses the attempt to a timing race the harness already handles
gracefully on the two other Quit-then-launch checkpoints. Fix: when 'override_quit' is in self.completed, treat a
still-running desktop as ObservationNotReady('wait for the previous app to exit after native Quit'), mirroring reopen;
optionally require 'override_quit' before launch_override so the structured path is the only one. Verify by reading the
launch branch of perform() and the dispatch() exception handling.

### Triage notes

- Recorded bucket: `other`.
- Suggested bucket (reviewer): other (maintainer release tooling; costs a brick-test attempt and a retry, no product or
  user data impact).
- Confidence: likely; unverified premise is how often a model tester calls launch_override without first polling
  versions despite the instruction; the fatal handling itself is confirmed from source.
- possible_cover: none
- proposed_drop: none

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g33/f1 (finding), group g33 "Maintainer tooling: brick tests and release-notes screenshots", findings log line 1.
