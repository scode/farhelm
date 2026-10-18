## What this was about

Host setup and uninstall confirmations used plain text that was hard to scan and did not name every action a run would
take. Progress then appeared separately in a host row, while refusals of several host actions appeared as red sidebar
text. The uninstall result also occupied several lines of the narrow hosts panel. The maintainer chose one helm-written
step list for confirmation and progress, a numbered dialog, progress in the host row, dialogs for definite refusals, and
a one-line uninstall result.

The six draft PRs implement that choice. Every planned action appears once under the same name in the confirmation and
run, with its paths and any explanation. Setup, Update and uninstall progress use the same rail; setup and uninstall
open their row for the run without changing the panel-wide Details setting, and a user can fold the row. The uninstall
result is one line with the kept-data path available on hover and through Copy. Definite setup, uninstall, Retry, adopt
and Remove refusals now appear in the dialog associated with the action.

## Things you should know

The helm remains the only author of the action wording. Copy plan returns its exact text, while the numbered view shows
shorter home-relative paths with the full paths on hover. A refusal during the helm's recheck after confirmation is
still a failed run in the host row, with the failed step and reason visible. Update submission refusals and host-list
read errors stay where they were. When a helm response is lost or cannot be read, Farhelm cannot tell whether the action
started, so it keeps a warning in the host panel instead of showing a definite refusal. The inline Update confirmation
also uses the numbered component, but normal Update submits without that confirmation; it is a defensive path.

Remove keeps its confirmation open until the helm replies and shows a refusal there. If the saved preference skips that
confirmation, a refusal opens a short dialog. A definite add-host setup refusal reopens the add form with its
destination; the used setup offer cannot be reused.

The Add a remote host setup-plan screenshot is stale: it still shows the old text-box confirmation. The changed Manage
hosts page describes the new behavior. This unattended run did not start a docs preview or recapture screenshots.

## Open questions and possible follow-ups

No decision is needed for this plan. A later docs screenshot refresh should recapture the Add a remote host setup-plan
image.

## PRs

- [#2298 — list all host actions in confirmations](https://github.com/scode/farhelm/pull/2298/changes): helm step
  wording and matching plan/progress data, draft.
- [#2303 — show host confirmations as numbered steps](https://github.com/scode/farhelm/pull/2303/changes): shared dialog
  stepper and exact-plan copy, draft on #2298.
- [#2305 — show host setup and uninstall progress in rows](https://github.com/scode/farhelm/pull/2305/changes): step
  rail and one-off row expansion, draft on #2303.
- [#2307 — keep host uninstall results on one line](https://github.com/scode/farhelm/pull/2307/changes): compact result
  with hover, Copy and dismissal, draft on #2305.
- [#2311 — show host planning failures in dialogs](https://github.com/scode/farhelm/pull/2311/changes): planning and
  confirmed-action refusals, draft on #2307.
- [#2317 — keep host action refusals in dialogs](https://github.com/scode/farhelm/pull/2317/changes): remaining
  host-action and add-host refusals, docs and three completed TODO removals, draft on #2311.

## Checks run, reused and skipped

Source checks run during the six PRs: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo check -p farhelm-ui --features desktop`, `dprint check`, changelog format, diff whitespace, and the isolated
test-delay check (304 delays, none missing a reason). All passed on the completed PRs. Rust tests ran through the
recorder with pinned nextest and tmux, four global slots and zero retries. The redacted command forms were
`python3 scripts/record-test-run.py --runner nextest --kind development --tmux required -- cargo nextest run -p farhelm-helm -E '<named provisioning cases>'`
for the helm and the same recorder form with `cargo nextest run -p farhelm-ui -E 'test(provisioning::) | test(hosts::)'`
for the UI; each run's manifest retains its exact selection and concurrency. The helm's exact plan/progress contract
passed 5/5 in 6851086a-6f22-4ff4-b38e-6499922e0ada. UI provisioning and host selections passed 57/57 for the dialog in
bbaab3d8-6784-4a6d-9763-3bd8f5136064, the corrected progress summary 1/1 in 4926d7b3-4a6b-47bd-ae28-1b72bc824bc7, the
corrected notice 2/2 in 086cff27-f0bf-4d61-b167-f6d843517d3d, and 60/60 each for the two refusal PRs in
53d4e147-e6ab-4164-a4fb-4c057b144cff and 6fec4922-5656-4f84-9b04-de04497afb7a. The final UI unit run preceded a
Remove-dialog lifetime fix; the browser check below covered that interaction.

Browser tests ran through the recorder with pinned tmux, one worker, zero retries, on Chromium and WebKit. The redacted
command form was
`python3 scripts/record-test-run.py --runner playwright --kind development --tmux required -- npx playwright test tests/provisioning.spec.ts tests/terminal-multihost.spec.ts -g '<named host cases>'`;
runs used one or both spec paths as the named selection required. Numbered confirmations, copy, layout and fallback
passed 8/8 in e69a6dcd-7324-45f2-80df-b95727a19154 and 2/2 in ab9f05d2-1033-49c5-b3ba-62b361e21035. Row expansion,
progress and folded status passed 10/10 in 5f4989b9-50d6-48cd-9859-9c8c5455010f and 4/4 after the review correction in
fc33cf3f-60bb-466a-8b59-f00a3de72318. The corrected compact notice passed 2/2 in 01c76968-5283-43f2-81dc-f210d7867658.
Planning and confirmed-action refusal coverage combines ten unaffected passes in abeca055-c413-43f7-9dcc-df179fed88ef
with 4/4 corrected cases in cb428ce6-040d-4009-9dc3-4888065b2d18. Retry, adopt, Remove and add-host coverage combines
twelve unaffected passes in 78d71c1e-ced0-4908-bfe0-f1b3bc45c7a6 with 4/4 corrected cases in
7907a8ba-43cb-489f-9888-be5bcaaa2967, including a held Remove reply and guarded dialog dismissal.

Retained failed attempts were same-session test errors, not latent flakes: 7a7db78a-3915-4e92-83cd-d05ca411cb35 did not
compile because of an invalid exact-text assertion; 0b60acb7-de4e-47ee-b9e8-b894d415502e had a stale step-name
assertion, fixed in the 1/1 rerun dad41047-75e0-4c3c-b2dd-de3b0b6bfb96; 4561e3bb-f506-47e7-b4e8-8debfcd69862 and
42037070-c3fe-46a0-9674-d63c36154e17 used wrong Details premises, route matching or old step names, fixed before the
passing progress runs; abeca055-c413-43f7-9dcc-df179fed88ef used an SSH fixture that refused planning before the held
submission the test meant to reach; and 78d71c1e-ced0-4908-bfe0-f1b3bc45c7a6 assumed Details started unchecked. Each
failure remains retained; later passes are separate observations.

The stack was rebased from 96b8f46d to main 5f1ff618 after six PRs had been drafted. Main's launcher and desktop
download changes use separate UI paths; its session-hover change shares tooltip code with the notice. The shared TODO
conflict was resolved by retaining main's entries and removing only this plan's three completed items. On rebased head
0ddc798e, `cargo build`, the release web build, desktop UI compilation, full-target Clippy, cargo formatting, dprint,
changelog format and diff whitespace all passed. Recorded run a703d6e0-bda9-48d8-a6f1-1c17ccbf5ffc passed the compact
notice hover, copy and phone-width case 2/2 on Chromium and WebKit against the changed tooltip code. The earlier focused
Rust and browser runs covered the original six PR heads through 7ae779e3; the rebase preserved their product behavior,
apart from this shared tooltip interaction, and the only conflict resolution changed TODO bookkeeping. The one later
main commit only delivered another plan's queue report.

Skipped: the full Rust and browser batteries offered no useful coverage beyond these focused runs and the rebased
tooltip case. The website build was skipped because the Manage hosts edit changed no links or headings. The desktop app
run was skipped because this stack changes the shared web renderer, which compiled with the desktop feature and ran in
WebKit; it changes no desktop shell or IPC path. The docs preview and screenshot capture were skipped because this run
is unattended and the screenshot refresh is a separate follow-up.

## Review gate

A fresh-context gpt-6.1-sol high reviewer examined each PR. Findings led to missing install paths and stronger
step-parity assertions in #2298; path whitespace and uninstall focus checks in #2303; warning detail and folded-status
wording in #2305; direction isolation for hostile hover text in #2307; modal handoff and disappearing-row handling in
#2311; and keeping Remove's modal isolated when its header × is clicked during a held request in #2317. Each finding was
addressed in its PR, then checked by the relevant focused tests. Commit and PR wording also passed a fresh cold read. No
review finding remains open.
