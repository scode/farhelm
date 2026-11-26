# Report: use the shared Delete confirmation for custom agents in Settings

## What this was about

In Settings → Custom agents, Delete asked for confirmation with two plain buttons built just for that page, unlike every
other Delete in the app, and it behaved badly:

- Escape could close Settings instead of cancelling the delete.
- Keyboard focus never moved to Cancel.
- The question never said which registration it would delete.
- While the question was open, the editor refused to save. Choosing Save in "Save changes before closing?" showed "Save
  was blocked. Choose Cancel to keep editing." and kept Settings open.

The plan records one decision, settled during planning: an accepted Save and Close should drop an unanswered Delete
question and save the draft, rather than refuse the save.

The PR moves this Delete onto the shared inline question that recipes already use for theirs:

- The question appears in the pinned footer, in the buttons' place. It reads "Delete this custom agent?" with "Delete
  ⟨name⟩ (⟨id⟩)? Existing sessions keep their saved resume command." beneath. It names the registration as it was when
  asked, and the delete goes to that registration's ID.
- Focus starts on Cancel, because a deleted registration cannot be brought back. Escape cancels the delete and keeps
  Settings open, and focus returns to the Delete button.
- A delete that cannot start (another save of the same registration in flight, say) keeps the draft. The reason appears
  in the footer.
- Accepting "Save changes before closing?" while the question is unanswered drops the question and saves the draft. The
  "Save was blocked" refusal is gone.

The PR also removes the TODO entry and adds a release note fragment.

## Things you should know

- The plan asked for no release note of its own, but every `fix` PR carries a changelog fragment, so the PR adds one. It
  says the Delete question now names the agent, starts on Cancel and lets Escape cancel it, and that saving on close no
  longer refuses while the question is open.
- One screenshot golden changed. Goldens are the reference pictures the desktop app's screenshot tests compare against.
  They live in a separate private store, and the repository keeps only a list of them (the manifest).
  - The old "Save was blocked by Delete" scene is gone.
  - A new scene, settings-custom-agent-delete, shows the open question in light and dark. I looked at both: the question
    and both answers fit in the footer and read correctly.
  - Its pictures are published to the store, and the PR's manifest lists them. No other golden changed in a full render.
- Save and Close behaves this way: the close question opens over the still-open Delete question, focused on its Save.
  Pressing Save drops the Delete question. Pressing Keep editing leaves the Delete question open.
- A small test helper was added for clicking a registered custom agent's own row in the Settings sidebar. The existing
  helper could name only the fixed sections.

## Open questions and possible follow-ups

None.

## PRs

- #5075 — fix: ask custom agent deletes with the shared inline question.

## Checks

Run now:

- `cargo nextest run -p farhelm-gui --lib` on the whole Settings module and the shared prompt's tests, on the final
  commit: 285 passed (recorded run 9b4e32fe-0724-4f67-8e50-b9b705116788). The updated custom-agent test covers:
  - the question's words, with the frozen name and ID;
  - focus on Cancel, and Escape returning focus to Delete with Settings still open;
  - Save and Close with the question unanswered, sending the draft's save and dropping the question;
  - a confirmed delete;
  - a delete refused because the registration had other work in flight.
- `cargo clippy -p farhelm-gui --all-targets -- -D warnings` and `cargo fmt -p farhelm-gui -- --check`: clean on the
  final commit.
- `python3 -B scripts/test-gui-screenshot-batch.py`, which checks the scene list against the manifest: passed.
- `dprint check` on the changed Markdown and JSON, and `python3 releasing/check-changelog.py format`: passed.

Reused, both run on the commit before the round-1 review fixes:

- `python3 scripts/check-gui-goldens.py --update`, the full golden render: passed, and its review page listed only the
  new scene (recorded run 3d4ea238-cffc-417d-9e47-ea304c10cfd7).
- After publishing, `python3 scripts/check-gui-goldens.py --scene 'settings-custom-agent*'`, comparing every
  custom-agent Settings scene against the published pictures: passed (recorded run
  57842dd0-20ed-4314-93bb-e6821f69ea88).

Their coverage still applies: the review fixes changed only when an error is cleared, added a footer message for a
delete that could not be built, and changed comments and a test, none of which the scenes draw.

Skipped:

- The workspace and end-to-end batteries: the change is confined to one Settings page, which the GUI tests above cover.
- The desktop app's end-to-end smoke: it has no Settings custom-agent step.

## Review gate

A single fresh-context Opus 5.5 reviewer at high effort, using the plan's general charter and the test-authoring
checklist, reviewed the PR in two rounds.

- Round 1 found four low-severity problems and nothing breaking the acceptance criteria. All four were fixed:
  - The docs wrongly said Recipes drops its Delete question on Settings' close save. Recipes drops it only for its own
    leave question, and the wording now says so.
  - A delete whose request could not be built failed silently. It now says so in the footer.
  - A registration removed elsewhere could clear an unrelated error. Now only an open question's removal clears it.
  - A test step could not tell the close question's Save from the Delete question's Delete. It now checks the label.
- Round 2 found nothing.

### Landing

#### What landed with it

This round landed bulk-start-sessions first (Restart/Replace for marked ended sessions in the session list), then this
plan. The two change different parts of the desktop app and met only in TODO.md, where the landing kept both removals.
Since this plan's base, main had also gained the Wayland text-field paste fix (#5069), the fix that judges a custom
agent's conversation report by the settings a "Restart with" just chose (#5071), and the fix that remembers each custom
agent's wrapper separately for each registration (#5073, #5074); while it landed, main gained a fix to the release brick
test's own harness (#5076). The wrapper fix is the one that touches custom agents. An independent reviewer and I read it
against this plan: it does not change how Delete asks the helm to remove an agent, or what the helm tells the app about
each agent, so the new question works the same on top of it. Nothing interacted.

#### Fixes made while landing

None. The independent review found nothing to fix.

#### Worth knowing

- The question says the same thing twice: "Delete this custom agent?" as its heading, then "Delete Example Code
  (example-code)? Existing sessions keep their saved resume command." beneath. It reads clearly in the new screenshot,
  but the heading and the first sentence could be merged if you prefer.
- The question remembers which agent it is about by its ID. If another Farhelm window or client on the same helm deleted
  that agent and registered a new one under the same ID while the question was open, confirming would delete the new
  one. The old two-button question had the same exposure, and it needs two windows editing the same agent at once.
- What landed goes against a rule in SPEC.md. The spec says nothing closes a confirmation it did not open, but accepting
  "Save changes before closing?" now drops an unanswered Delete question, as the plan's decision asked. The spec names
  no exception for it. Either SPEC.md gets an exception for the close save, or the behavior goes back to keeping the
  Delete question open; the first matches the plan's decision.

#### Checks

Run once for the whole round, on bulk-start-sessions and custom-agent-delete-prompt stacked in that order on main as it
stood when the landing started (`bf468652`); what main gained since each plan's base is in each plan's notes. The brick
test harness fix (#5076), which reached main while this round was landing, is not exercised by any check here. The test
worker is an aarch64 Linux machine:

- On the test worker, the four clippy configurations (`cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, and `cargo clippy -p farhelm-desktop --all-targets` with and without
  `--features agent`, each with `-D warnings`): clean.
- On the test worker, `cargo nextest run -p farhelm-gui -p farhelm-desktop`, recorded run
  bd36cea5-d4a2-4942-86ca-981f0eee85fb: 3,692 passed, 4 skipped. That is every desktop app test, wider than either
  executor's selection.
- On the test worker, the desktop app's end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), recorded run
  25abe428-bee0-437b-9486-34e05898acf1: all 49 steps passed, except the paste step, which runs only on a Mac and was
  skipped as always on Linux. It marks no sessions, but it exercises the row menus' Stop, Restart and Replace and
  Settings opening and closing.
- Locally, every screenshot scene compared with its approved picture, with both plans' new and changed screenshot scenes
  included (`python3 scripts/check-gui-goldens.py`), recorded run 9bb96428-674a-4cb0-98e1-6be02b626c7e: every picture
  matched, the new custom agent Delete scene included.
- Locally, `python3 -B scripts/test-gui-screenshot-batch.py`, which checks the screenshot scene list against the golden
  manifest, recorded run 26eeb3f3-b1a9-43bc-a31e-63b8e02b4697: passed.
- Locally, `cargo fmt --all -- --check`, the test-sleep check, `python3 releasing/check-changelog.py format` and
  `dprint check` on the changed Markdown and the scene list: passed.

Reused: nothing. Skipped: the helm's, the supervisor's and the end-to-end Rust tests, since both plans change only the
desktop app and nothing it sends.
