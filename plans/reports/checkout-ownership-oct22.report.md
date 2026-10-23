# Report: five supervisor fixes around managed checkouts, prepared checkouts and Delete

## What this was about

Five review findings from the 2026-10-22 triage, all about the supervisor acting on the wrong folder or losing a
surviving session's files. You accepted each recommendation on the triage page; this plan built one draft PR per
finding, in the order the triage ledger lists them, each under its own size gate of about 100 lines.

1. **A re-cloned folder treated as Farhelm's own.** Farhelm tells a checkout it created from a stranger at the same path
   by the folder's identity number and its creation time. On a filesystem that records no creation time it fell back to
   identity number and device, and a folder the user deleted and cloned again at the same path can get the same pair
   back. Deleting the session then moved the user's folder into the checkout archive, where the trash could later delete
   it for good, and a new session started in it skipped the agent's "do you trust this folder" question. Now, when
   neither Farhelm's record nor the folder has a creation time, the checkout counts as unconfirmed: Delete releases it
   without moving it and shows the notice it already gives when a folder cannot be archived, the trash never deletes it
   (it lists the entry as left untouched, and Delete there only drops the entry), and a new session in that folder gets
   the ordinary trust question. One case is unchanged: a checkout Farhelm recorded before it started storing creation
   times, whose folder reports one now, is still compared by identity number and device alone. That was accepted earlier
   for such old records, since they cannot be checked any other way.
2. **A late create retry taking over a restarted session.** A client that never heard whether its create worked can
   retry it. If the session looked as though its launch never started, the retry deleted the session's record, inserted
   it again and launched again, even when the user had restarted the session in the meantime and the restart's outcome
   was itself unknown. Every restart advances the session's launch generation, which a create leaves at zero, so a retry
   now treats any session past generation zero as already launched and answers with the existing session. Found by
   reading the code; what it would cost a user in practice was not established, and it was not reproduced.
3. **Attachments lost after an interrupted Delete.** Delete parks a session's attachments (screenshots and other
   uploads) in a holding folder before removing the session. If the supervisor stopped in between, or a failed Delete
   could not move them back, the session survived but the next startup emptied the holding folder. Startup now moves a
   surviving session's parked attachments back, keeps them parked if that cannot be done safely, and deletes only those
   whose session is gone.
4. **A duplicated tmux row releasing a checkout still being prepared.** While a new session's launch may still be
   cloning its checkout, the supervisor protects that checkout from cleanup; at startup it decides which of these
   protections still hold by checking whether each launching session's tmux pane is alive. A pane listed twice (which
   needs a newline in a session name) was dropped from the listing and read as dead, so the checkout could be cleaned up
   mid-clone. You chose this cheap fix over closing the item as covered by Farhelm's private tmux server. Startup now
   treats such a listing as uncertain and keeps every checkout protected, as it already did when the tmux query failed.
   Everything else that reads the pane list keeps dropping just that one pane.
5. **A prepared checkout keeping history upstream removed.** A launch can reuse a checkout Farhelm cloned earlier, by
   fetching and fast-forwarding it. If the branch had been force-pushed back to an older commit, the fast-forward
   succeeded without moving, so the session started on commits upstream had removed; if it had been rewritten some other
   way, the fast-forward failed after the move and stopped the launch. The launch now checks, before moving anything,
   that the prepared commit is still in the fetched branch's history, and otherwise clones normally, as it already did
   for a branch deleted upstream. The spec, the implementation notes and the Start a session docs page say so.

## Things you should know

- **Four of the five PRs are at or slightly over the 100-line gate; I accepted them as "about 100" rather than
  blocking.** Measured code, test and doc lines: fix 1 (#2634) 108, fix 2 (#2638) 108 with 48 of them deletions, fix 3
  (#2640) 117, fix 5 (#2647) 101; fix 4 (#2645) is 69. In each, the fix itself is well under the gate and the rest is
  documentation and test cases the review gate asked for. If you want the gate treated as a hard limit in future plans,
  that is a note for the next planning round.
- **Fix 1 was narrowed on review.** It covers Delete's archive, the trash, and trust for a new session started in an
  existing folder. An earlier draft also made Claude ask its "do you trust this folder" question in a checkout Farhelm
  had just cloned; the review pointed out that Farhelm made that folder itself, so that part was reverted.
- **Fix 2 closes the takeover on both paths a retry can take.** Besides the takeover itself, the retry first checks
  whether the launch already happened, and before this change it could go on to re-validate the restarted session and,
  if that validation refused (say its checkout folder had become unusable), record a create failure over the restarted
  session. That check now also treats a restarted session as already launched.
- **Fix 4's first design was replaced.** Failing the whole pane listing on a duplicate would have stopped the supervisor
  from starting, and broken the session list and Delete on that host, for as long as such a name existed. The landed
  shape only affects startup's protection check. tmux at Farhelm's minimum version refuses a newline in a session name,
  so the trigger cannot be produced today: this is defence in depth, and its changelog fragment says nothing reaches
  users. A unit test covers the duplicate detection; that startup uses it is visible in the code but not tested, because
  real tmux cannot produce the input and a fake tmux for one unproducible case is not worth building.
- **Fix 3 leaves a parked folder in place when its session has new attachments since the failed Delete.** Merging could
  mix or overwrite files, and deleting it is the loss being fixed, so it stays parked and the supervisor's log gets a
  warning about it at each startup. The user sees nothing.
- **Fix 5 can fall back to a full clone on a very large fetch.** The new history check shares the one-second limit
  Farhelm puts on its local Git queries. If a prepared checkout is far behind a busy repository, the check can time out
  after the fetch, and the launch then clones normally: slower than before, but on the right commit. This is documented
  in the implementation notes.
- **Fix 5 edits the unreleased prepared-checkout changelog entry** rather than adding a fragment, since prepared
  checkouts have not shipped yet.

## Open questions and possible follow-ups

- **"Restart with" in a re-cloned folder still skips the trust question.** On a filesystem without creation times, if
  the user deletes a Farhelm checkout and clones their own repository at the same path, then uses "Restart with" on the
  old session, the agent starts without asking whether to trust the folder, because Restart with decides trust from the
  session's own checkout record rather than the folder. Fix 1's scope covered new sessions only. Recommendation: a small
  follow-up giving Restart with the same unconfirmed check, since a trust prompt skipped in a folder Farhelm did not
  create is the exact problem fix 1 addresses.
- **An archive move already interrupted before this change still completes under the old comparison.** If a Delete was
  archiving such a checkout when the supervisor stopped, startup finishes that move on its next run using identity
  number and device only. The worst case is fix 1's loss (the user's re-cloned folder archived and later deleted by the
  trash), but it needs that interrupted Delete, a re-clone at the same path before the next startup, and a filesystem
  without creation times. Recommendation: leave it; new interrupted moves cannot arise after this change.
- **A parked attachments folder that stays parked (fix 3) is never cleaned up.** It costs its disk space and one log
  warning per startup, and nothing tells the user. Recommendation: leave it unless it is reported; surfacing or expiring
  it would be new behavior for a rare double failure.

## The PRs

- #2634 — leave checkouts without creation times unarchived and untrusted (fix 1)
- #2638 — keep a create retry from taking over a restarted session (fix 2)
- #2640 — give a surviving session back its attachments at startup (fix 3)
- #2645 — keep startup checkout claims when a tmux pane is listed twice (fix 4)
- #2647 — clone normally when a prepared checkout's branch was rewound (fix 5)

## Checks

Run now, after rebasing the stack onto the current main: `cargo fmt --all -- --check` and
`cargo clippy --all-targets -- -D warnings` on the top of the stack, both clean.

Reused: each PR's tests ran through the test recorder with the pinned nextest and tmux on that PR's own commit before
the final rebase. The rebase brought in only browser-side sign-in and preference-save fixes, their spec wording and plan
bookkeeping, none of which touches the supervisor code or spec sections these PRs change, so the runs still apply:

- fix 1: `cargo nextest run -p farhelm-supervisor --lib` filtered to working copies, checkout trash and teardown, 98/98
  (run `1282710d-cba9-4338-b038-91e1548a54da`).
- fix 2: the same, filtered to the store and create retries, 140/140 (`f9b0d1e2-44ad-4dc7-bd53-39a3cffaf326`), and the
  service core tests, 234/234 (`3daf2a4c-c2e4-499f-af0a-aa8bbd9ff91d`).
- fix 3: attachments and teardown, 40/40 (`f432ba34-c9df-4be5-9796-a0a454d3eb65`).
- fix 4: tmux, status and startup protection tests, 173/173 (`bee91a96-c8aa-4db9-9f28-f467e0bbdc57`).
- fix 5: prepared-checkout tests, 50/50 (`5ce0976f-3845-460d-908a-bd96a0ec4688`); the new test also ran against the
  unfixed code and failed as expected (`4af9d15a-47a7-42de-af97-e1e1ae0c7f83`).
- `python -B scripts/check-test-sleeps.py`, `python3 releasing/check-changelog.py format`, and `dprint check` on the
  changed Markdown, all clean on the stack's top commit before the rebase, which changed none of the files they read in
  these PRs.

Skipped: browser tests (nothing in the browser UI changed) and the full Rust battery (each change is local to the
supervisor modules its tests cover).

## Review gate

One fresh Opus 5.5 reviewer at high effort per PR, as the plan asks.

- Fix 1: narrowed as described above. Two findings not followed: the interrupted-archive case, now a follow-up, and
  extra tests pinning how the new refusal turns into Delete's notice and the trash's "left untouched"; those paths are
  short and visible in the code, and the tests would have pushed the PR further over the gate.
- Fix 2: all findings accepted, including the second path described above and a correction to an existing test that the
  first draft would have broken.
- Fix 3: all accepted.
- Fix 4: the first design was replaced after review; a second review of the replacement found no correctness problems,
  only documentation, which was fixed.
- Fix 5: no correctness problems; its wording and test findings were applied, and the one-second limit was documented
  rather than given its own longer deadline.
