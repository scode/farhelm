# Open inline Delete confirmation in Sandboxes survives Tensorlake API key replacement or clear

Reviewed commit: de4302008ae668ca4ee7f569e7fad36e55521285

## TLDR

If you open the inline Delete confirmation for a leftover sandbox in Settings → Tensorlake and then clear or replace
your Tensorlake API key (either on that page or in Settings → Credentials), the confirmation prompt for the old key's
sandbox stays open — keeping the Sandboxes section visible even after Clear, or letting you confirm deleting an old-key
sandbox under the new key.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/settings/tensorlake_sandboxes.rs:154-175`.

`Sandboxes::ask` (`crates/gui/farhelm-gui/src/settings/tensorlake_sandboxes.rs:273-319`) opens the Delete confirmation
via `prompt::ask_inline`, which has no modal veil (`crates/gui/farhelm-gui/src/prompt.rs:111-112`) and persists while
`Sandboxes` stays alive across section switches in one Settings opening
(`crates/gui/farhelm-gui/src/settings/tensorlake.rs:97-99`). When the user clears or replaces the API key,
`Sandboxes::follow_account` (`tensorlake_sandboxes.rs:154-175`) updates `self.key` and (on Clear) empties
`self.last_discovered`, `self.cleanups`, `self.discovery_error`, and `self.checked_at`, but never clears `self.question`
or `self.asked` (nor `self.delete_failure`). Because `Sandboxes::drawn` (`lines 374-381`) returns true whenever
`self.asking_about(cx).is_some() || self.delete_failure.is_some()`, clearing the key while a Delete prompt is open
leaves the "Sandboxes not on this helm" region drawn with the stale Delete prompt for the cleared key's sandbox;
replacing the key leaves the old-key confirmation open alongside the new key's list (and confirming it sends
`DELETE /api/tensorlake/orphans/{id}` under the new key, which interacts with `l7` if that ID was an unfinished cleanup
from the old key). Dropping `self.question` takes an inline prompt away unanswered (`prompt.rs:1370`). How to verify: in
`tensorlake_sandbox_tests.rs`, open a Delete confirmation on `sb-1`, press `tensorlake-key-clear` (or save a replacement
key), reply 200 to `PUT /api/tensorlake`, and assert `drawn(cx, "prompt-confirm")` — the prompt remains open. Fix: in
`Sandboxes::follow_account`, whenever `self.key` changes from a previous `Some(_)` state (on both Clear and key
replacement), set `self.question = None; self.asked = None; self.delete_failure = None;`.

Reviewer metadata (g16_tensorlake_leftovers_discovery/l8): suggested bucket: other — stale confirmation prompt surviving
key clear or replacement; on Clear the helm refuses the Delete for missing key, and on replacement `delete_orphan`
checks canonical ID (with cross-account failed-cleanup retry risk tracked in `l7`), so there is no security or live-data
loss route.; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g16_tensorlake_leftovers_discovery/l8`.
