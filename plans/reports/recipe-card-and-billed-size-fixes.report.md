## What this was about

A removed container host could remain on a recipe card indefinitely if a build had failed there. Re-adding the host gave
it a new identity, so the old row stayed unusable. The card now drops removed hosts whose only record is a past failure.
Registered hosts still show their failure history. Removed hosts with active work or resources to clean up stay visible.

Previously, an edit to a Tensorlake recipe while the launcher's billing question was open could start a sandbox with a
different CPU, memory or disk size than the user had agreed to pay for. **Create and launch** now confirms the size
shown in the question. If the helm reads a different size at admission, it refuses before registering a host or
contacting Tensorlake. The launcher keeps the draft and shows the updated size when you press Launch again.

## Things you should know

Confirmed launches through the current environment route require a desktop app and helm that both support the size
field. In a mixed-version setup the launch is refused, with the draft still available. Clients of the legacy sandbox
endpoint can still send the confirmation flag alone. The deliberate Tensorlake skip setting and ordinary container
launches still send unconfirmed requests.

## Open questions and possible follow-ups

None.

## PRs

- #4980 hides removed container hosts with only a past build failure.
- #4988 binds Tensorlake launch confirmation to the size the launcher showed.

## Checks run, reused and skipped

- Recipe-place tests covered registered failures across recipe bases, omission of removed failure-only hosts, retained
  resources, active work and the recipe feed. All three passed in the helm test binary built from `ee2a91ca`'s source.
  Ran recorded
  `cargo nextest run -p farhelm-helm --lib -E 'test(/ui_channel_tests::.*recipe/) | test(a_place_jobs_admission_step_and_end_reach_the_connected_ui)'`.
  The module filter missed two cases, so ran those exact tests using the same compiled libtest binary,
  `recipe_places_keep_usable_history_and_single_provider_places` and
  `a_recipe_edit_reaches_the_connected_ui_and_the_reads_start_nothing`. Recorder IDs
  `5fb1c579-28fe-4ee3-9f10-7756687cf8a9` and `43fcc409-1c9c-4987-8b70-0eaabf386a22`.
- The focused helm and launcher run passed 38 tests against `290edcc0` plus a capture-only fixture. It checked a
  same-name edit before confirmation, refusal before registration or provider calls, matching-size creation, the legacy
  route, the skip setting and ordinary container launches. Ran recorded
  `cargo nextest run -p farhelm-helm -p farhelm-gui --lib` with filters for the sandbox and environment launcher tests,
  recipe session provisioning, request decoding and the container route. Recorder ID
  `3f7197f7-10ed-4d47-a3c9-181a9ccba2f2`.
- Inspected Linux light and dark captures from `af8c80b0`, the same product code with a temporary capture fixture. Both
  show the complete refusal, retained draft and enabled Launch button. Ran recorded
  `python3 scripts/check-gui-goldens.py --update --scene launcher-new-sandbox-checkout --jobs 2`. Recorder ID
  `0f277aba-48b2-46ac-983d-a094916db13a`.
- Reused recorded runtime and visual evidence on the final draft heads `13371629` and `3662ae60`. The rebase preserved
  both patches. The intervening code changes cover existing sandbox setup and repository-cache cleanup, outside the
  recipe feed, fresh session launch path and launcher behavior tested here.
- `cargo fmt --all -- --check`, targeted `dprint check`, `python3 releasing/check-changelog.py format` and
  `python -B scripts/check-test-sleeps.py` passed. The workspace battery and a live Tensorlake launch were skipped
  because the focused provider and GUI fixtures directly exercised the changed behavior.

## Review gate

Independent source review caught a recipe card regression when a registered host's build failure predated a base change.
The fix preserves that row and adds a test for it. Review of the consent change prompted a matching-size launch test
through the current route, a stronger check that the catalog edit arrived before the user answered, and documentation
corrections. The follow-up reviews found no remaining source issues.

### Landing

#### What landed with it

Only this plan landed this round, through its two PRs. Since the plan's PRs were written, main gained the tab switching
shortcuts and, while this landing ran, two TODO entries and a review queue wording fix, none of which touches this. The
plan's first PR reverses a behavior an earlier round landed: the plan that hid container hosts a shared recipe cannot
use had made a removed host with only a failed build record stay on the recipe card for good, and its test asserted
that. Now a removed host stays on the card only while the recipe still has running work or resources to clean up there,
while hosts still registered keep their failure rows; the earlier test is the one this PR rewrote, as the plan required,
and the rewritten test fails without the fix.

The landing's review checked the second PR's binding of a sandbox launch to the size its confirmation showed: the
launcher stores the CPU, memory and disk it shows in the question and sends exactly those, and the helm compares them
with the recipe it is about to create the sandbox from, before it registers a host or calls Tensorlake, so an edit to
the recipe while the question is open cannot bill for more than the user saw. New, Clone and quick launches all go
through that path; running setup again on a session's own sandbox keeps the size confirmed when it was first launched;
and launches that skip the question by the Tensorlake setting, and container launches, send no size, as the plan
decided. The protocol needed no version bump, because the desktop app and its helm are always the same build. The review
counted no blockers, one should-fix (fixed below) and four small points: the older helm wording under "What the report
says that is no longer true", and the agent approval card and the two test points in the last section.

#### Fixes made while landing

- **The size fix was marked as a breaking change it is not (the review's should-fix).** Its title carried the
  breaking-change mark and its changelog draft said that the desktop app and the helm both need the update and that API
  clients must now send the size. The helm only ever runs inside the desktop app at the same version, and nothing
  outside the app can reach that route, so there is nothing for anyone to do and no mixed-version case. I changed the
  title to a plain fix in the plan's own PR and rewrote the draft as a fix entry naming only what users see.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. All the checks below ran
on the plan's PRs rebased onto main as it stood at the start of the landing (`824c10fa`); the rebase conflicted only in
TODO.md, where both sides removed entries and I kept both, and every other file's change is exactly what the executor
pushed. The rebased stack's top commit was `32805151`. The only later change is in the plan's own second PR: its title
lost the breaking-change mark and its changelog draft was rewritten (see the fixes above); after it I ran
`python3 releasing/check-changelog.py format` again, and it passed. That final tree is what main now holds outside the
plans queue, apart from two TODO entries (#4993, #4994) and a review queue wording fix (#4984) that landed on main while
this landing ran and touch no code.

- The workspace Rust suite, `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `b89a66ad-2470-486a-929e-85dc31e0278d`): all 8713 tests passed, with no runtime skip. The plan changed the helm's
  recipe place list, sandbox session creation, Tensorlake setup and agent requests, the protocol's session request, and
  the GUI launcher's sandbox question, with their tests; the whole suite was the narrowest run that covered them and
  their callers.
- The desktop app's tests, `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run
  `374fe615-88ad-44ba-b1ef-b52837cd7069`): all 184 passed.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings`, on the test worker: passed.
- The desktop app's end-to-end smoke, `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run
  `c895d9d5-efeb-4fab-a865-580f479462e9`): 50 steps passed, one skipped by design on Linux (paste).
- The desktop app's screenshot comparison, `python3 scripts/check-gui-goldens.py`, all scenes, locally (recorder run
  `f05dc4af-5b2b-4df8-b120-b6cebbcc8b0c`): passed, so the sandbox question looks as before.
- The screenshot batch tool's test, `python3 -B scripts/test-gui-screenshot-batch.py`, locally (recorder run
  `bdd65470-622d-4e10-8ed7-8b17a339a444`): passed.
- `cargo fmt --all -- --check`, `python3 releasing/check-changelog.py format` and
  `python -B scripts/check-test-sleeps.py`, locally: passed. `python3 scripts/check-agents-md.py` and `dprint check`,
  locally: failed, each on a problem already on main that this plan does not touch (a review queue finding that cites a
  missing heading, which #4984 fixed on main while this landing ran, and unwrapped triage archive entries from the
  review queue triage record #4978, which main still has).

Reused: none. The executor's runs were focused selections on an older base, and everything they covered ran again above.

Skipped: a live Tensorlake sandbox launch, which needs a provider account; the workspace doctests, since no
documentation example changed; a Mac run, since nothing changed is Mac-specific; and CI, which runs only on demand.

#### What the report says that is no longer true

- The report, like the old changelog draft, describes an older helm refusing the new launcher's confirmation. That
  pairing cannot happen, since the helm ships inside the app; the code's handling of it is harmless, and a SPEC_impl.md
  sentence describes the same case. I recommend leaving both; they cost nothing.
- The report says clients of the older sandbox route can still confirm with the flag alone, without the size. That is
  true of the helm, but nothing in the app calls that route, and only the app's own window can reach the helm, so the
  size check cannot be skipped in practice. Removing the flag-only path would only tidy the helm; say if you want a TODO
  entry.

#### Things you should know that the report does not say

- **An agent's request to launch a sandbox still shows no size on its approval card and is not bound to one.** That is
  what the plan decided for the agent path, so it is a possible follow-up rather than a regression: the card cannot show
  the user what the sandbox will cost, and a recipe edit while the card waits changes the size. Say if you want a TODO
  entry.
- **Small test points.** The test of a size that changed while the question was open checks only the refusal's status
  codes, so an unrelated refusal with the same status would also pass; and the template quick launch test does not check
  that the size is sent, though that path shares the launcher's code today. Neither is worth a follow-up on its own.
- **Main still fails `dprint check`.** Triage archive entries from the review queue triage record (#4978) are not
  wrapped as the Markdown formatter wants. This plan does not touch them; a formatting-only PR fixes it, and I recommend
  one; say if you want me to open it. The other problem the checks met, a review queue finding citing a missing heading,
  was fixed on main by #4984 while this landing ran.
