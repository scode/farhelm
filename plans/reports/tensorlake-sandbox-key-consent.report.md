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
