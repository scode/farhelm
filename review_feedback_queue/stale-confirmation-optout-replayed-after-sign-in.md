# A browser's failed opt-out of confirmations is replayed over a later restore after it signs in again

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

Suppose you turn off session confirmations (or the question before removing or setting up a host) in one browser tab,
that save fails without telling you, and you later turn confirmations back on from the desktop app or another tab. The
first tab pushes its old opt-out back to the helm the next time it has to sign in again. After the other window next
reloads, Delete, Replace, Restart and closing a terminal tab run on the first click everywhere, though your most recent
choice was to be asked.

## Details

Found by the 2026-10-22 prioritized persistent review, group `176-ui-lib-3of6` (finding f1, as revised by the reviewer).

- `store_preference` (`crates/farhelm-ui/src/api.rs:1311-1316`) records, for example,
  `PreferenceValue::SessionConfirmations(true)` in the process-wide write queue. Its doc says failures "are logged here,
  not surfaced". A failed PUT leaves the value dirty with no retry and no expiry (per the reviewer: `finished`, api.rs
  around 1132-1151, and `dirty`, around 1216-1242).
- `seed_with_local_changes` (api.rs:1326-1362) runs on every reseed of the preferences gate, including after browser
  credential recovery. It overlays every dirty field, `SessionConfirmations`, `HostRemoveConfirmation` and
  `HostSetupConfirmation` included, onto the freshly fetched row and starts a replay write, without comparing with the
  fetched value or knowing whether the row changed since the failed write. Its doc treats the fetched row as predating
  the local choice, which is not true when another client wrote in between.
- Per the reviewer, the helm applies the patch as an unconditional per-field overwrite
  (`crates/farhelm-helm/src/store.rs` around 5133-5160,
  `skip_session_confirmations = CASE WHEN ?20 THEN excluded.skip_session_confirmations ...`, and the same for the two
  host switches). Recording did not reread this.
- The code already treats the restore direction as safety-relevant: `restore_session_confirmations` (api.rs around
  1431-1438) waits for an acknowledgment, and TRIAGE_OUTCOMES.md "confirmation-restoration-failure-reloads-optout.md"
  made a failed restore visible for this reason. The replay brings back an older opt-out from another client without any
  notice.

Scenario: browser tab A turns on "skip session confirmations" while the helm is briefly unreachable or refusing (for
example just after the browser sign-in token was rotated), so the PUT fails and stays dirty. The user restores
confirmations from the desktop app; that write succeeds. Tab A later goes through the token prompt, and its reseed
replays the opt-out. The other client keeps asking until it reloads, then skips the questions SPEC.md lists for that
switch (around 1262-1265). For the host switches the effect is host removal or setup without its question.

How to verify: unit-level on `PreferenceWrites`: record `SessionConfirmations(true)`, finish it as failed, then call
`seed_with_local_changes` with a seed holding `Some(false)` and observe the overlay to true and a replay claim. End to
end: two clients, block A's PUT, restore from B, rotate the token, sign A in, reload B and delete a session.

Fix shape: do not replay a value that reduces confirmations on reseed; or drop a dirty value once a reseed shows the
helm holding a different value than the one this client last saw; or replay only values recorded since the last
successful seed.

Suggested bucket: high. It might be argued highest, since destructive actions then run without the confirmation the user
last asked for, but each still needs the user's own click, and the supervisor's precondition checks and YOLO questions
still apply.

Confidence: likely. The client path is confirmed by reading; the helm overwrite was read by the reviewer; not
reproduced.

Reviewer's proposed drop, kept for triage: SPEC.md "One GUI at a time": "Several GUIs open on the same helm at once work
on a best-effort basis: an action taken in one can race a change made in another, and the outcome may reflect what that
GUI last showed rather than the newest state." SPEC.md "Signing in again" makes browser recovery best effort. The
reviewer's counterpoints, which is why recording did not drop it: the replay can come arbitrarily long after the race,
it rewrites the durable row every client reads, and "One GUI at a time" says "destructive prompts' preconditions still
hold". "Signing in again" accepts losing reports, not reinstating an old setting.

Possible cover, for triage to judge: SPEC.md "One GUI at a time"; SPEC.md "Signing in again"; TRIAGE_OUTCOMES.md
"confirmation-restoration-failure-reloads-optout.md" (a related precedent with a different trigger; it does not cover
the replay). FILTER.md "Races a person would have to win inside a sub-second window" does not match: there is no
sub-second window.
