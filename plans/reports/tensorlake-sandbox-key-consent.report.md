### What this was about

Settings → Tensorlake lists "Sandboxes not on this helm", leftover Farhelm sandboxes in the Tensorlake account, each
with a Delete that asks first. The list and its Delete question belong to the API key that found those sandboxes, but
they could outlive that key. If you saved a new key or cleared the key, in either Settings → Tensorlake or Settings →
Credentials, an open Delete question stayed open. Changing the key in Credentials hid the question, and coming back to
Tensorlake showed it again, still open, next to the old key's rows. Confirming it sent the Delete for a sandbox the
current key never listed. The helm looks the sandbox up again with the current key, so if the new key could see that
sandbox (another key for the same Tensorlake project), it was deleted; if not, the helm refused it. A Delete still in
progress when the key changed also showed its result under the new key's list. Clearing the key while the question was
open left the question on screen under an otherwise empty list. This was a follow-up from triage; your one recorded
decision was to keep it simple and not add any special handling for a slow helm, since the helm looks the sandbox up
again with the current key when it deletes.

Now any save or clear of the key, from either form, closes the Delete question and drops the old list at once, before
the new key's check answers. That also holds when the helm's answer to the save cannot be read: Settings then treats the
key as changed, stops showing the old list, and loads the account again before checking the new key. A Delete still in
progress when the key changes finishes quietly; the fresh check that follows the key change shows what it did.

### Things you should know

- When the helm's answer to a key save cannot be read, Settings → Tensorlake's other controls (the key form and the
  "create without asking" switch) are briefly locked again while the account reloads, as they are when Settings first
  opens. The helm always answers with its settings, so this only happens with a helm the app does not understand.
- No drawing changed, so no screenshots were re-taken.
- The implementation notes in SPEC_impl.md ("Tensorlake recipe tunnels and unowned sandboxes") said only that Clear
  empties the list; the PR rewrites that to describe the new behavior. SPEC.md needed no change.
- No changelog entry was planned for this plan, but repository rules ask every fix to add a draft entry, so the PR adds
  one (the list shipped in v0.45.0). Drop it at curation if it is too minor.

### Open questions and possible follow-ups

None.

### PRs

- #5077 fix: retire leftover sandbox Delete when the Tensorlake key changes

### Checks

Run now, all through the test recorder:

- Six new tests (including the companion test added after the second review), written before the fix, each failed on the
  unfixed code at the assertion that describes the bug (runs f6222803, d3f5bd27 and 55fcea32; the last run swapped the
  two changed source files back to main's text for two of them, then restored them).
- All Settings tests (`cargo nextest run -p farhelm-gui --lib -E 'test(/^settings::/)'`): 249 passed before the rebase
  (run 384e91fd) and again after it on the rebased PR head (run 82936407).
- `cargo clippy -p farhelm-gui --all-targets -- -D warnings`, `cargo fmt --all -- --check`, the test delay checker (306
  delays inspected, none lacking a rationale), `dprint check` on the changed Markdown, and
  `python3 releasing/check-changelog.py format`: all clean, on the rebased PR head.

Reused: none.

Skipped: the GUI screenshot comparisons, since no drawing changed; the full workspace battery, since the change is
confined to two Settings views in the desktop app and their tests. main had moved by five fix PRs, among them a change
to the GUI test platform's clipboard. None touches this code, and the Settings tests were run again after the rebase.

### Review gate

Two rounds, each a fresh Opus 5.5 reviewer at high effort with the plan's general charter. Neither found a bug in the
fix. The first asked for tests of clearing the key and of the new key's list showing a leftover whose earlier cleanup
did not finish, a small fix so a stale "couldn't load" message is dropped with the reload, and doc updates; all done.
The second found that one new test could not tell the old behavior from the new for its first claims, and that two test
descriptions gave the wrong reason; it got a companion test that does fail on the old code, and the descriptions were
corrected. The gate passed after the second round.

### Landing

#### What landed with it

Only this plan landed this round. Since its base, main had gained the Restart/Replace action for marked sessions (#5065,
#5072) and the change that makes Delete for a custom agent in Settings ask its confirmation in place, the same way this
Delete does (#5075). The second is also in Settings, so an independent reviewer and I read it against this plan: its
rule that accepting "Save changes before closing?" drops an unanswered Delete question lives in the custom agent editor
alone, not in anything the Tensorlake page shares, so nothing interacted. We also confirmed the plan kept to your
decision not to handle a slow helm specially. The Delete question closes when the helm accepts the key change, not when
you press Save. While the save is still going, or if the helm refuses it, the question stays open. That is safe because
the helm checks the key it has stored again when it deletes.

#### Fixes made while landing

Two code comments were rewrapped to the repository's line length. No behavior changed.

#### Worth knowing

- One check in the test where you confirm a Delete while a Clear of the key is still going proves nothing: it looks for
  the Delete question to be gone, but confirming the Delete had already closed it. This is not the weak test the review
  gate found and fixed. The test's other checks still fail on the old code, as the report says of all six tests, and a
  separate test covers a Clear closing a question that is still open, so the behavior is covered.
- Nothing tests a key change while keyboard focus is on the Delete question's buttons. On a Mac, clicking a button does
  not move keyboard focus, so focus can still be on the question when you click Clear. Settings already moves focus
  somewhere sensible when the control holding it disappears, and that is expected to cover this case, but nothing checks
  it here. Not worth a follow-up unless focus is seen getting lost.

Nothing from the landing needs a follow-up.

#### Checks

Run on the plan's change as the executor pushed it, rebased onto main as it stood when the landing started. The landing
fix above only rewraps two code comments, so these runs still cover what merged. The test worker is an aarch64 Linux
machine:

- On the test worker, the four clippy configurations (`cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, and `cargo clippy -p farhelm-desktop --all-targets` with and without
  `--features agent`, each with `-D warnings`): clean.
- On the test worker, `cargo nextest run -p farhelm-gui -p farhelm-desktop`, recorded run
  e6730559-2947-4ddb-8b9a-5dab100fdfaf: 3,698 passed, 4 skipped (tests marked ignored). That is every desktop app test,
  wider than the executor's Settings selection, and on the base that includes the Settings change that landed meanwhile
  (#5075).
- Locally, `cargo fmt --all -- --check` (again after the landing fix), the test-sleep check,
  `python3 releasing/check-changelog.py format` and `dprint check` on the changed Markdown: passed.

Reused: nothing. Skipped: the screenshot comparison and the end-to-end smoke, since nothing drawn changed and the smoke
has no Tensorlake step; the rest of the workspace, since only the desktop app's Settings changed.
