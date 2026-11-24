# Replacing the saved Tensorlake API key leaves the previous key's discovered sandboxes visible

Reviewed commit: de4302008ae668ca4ee7f569e7fad36e55521285

## TLDR

When you replace your saved Tensorlake API key with a different key in Settings without clicking Clear first, the list
of sandboxes not on this helm keeps showing leftover sandboxes from the old key while the new check runs — and keeps
showing them indefinitely with active Delete buttons if the new key fails to list sandboxes (for example, if the new key
is invalid or Tensorlake is unreachable).

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/settings/tensorlake_sandboxes.rs:154-175`.

In `crates/gui/farhelm-gui/src/settings/tensorlake_sandboxes.rs:154-175`, `Sandboxes::follow_account` tracks
`KeyState = (settings.api_key_set, account.key_saves())`. When the user replaces an already-saved key (e.g. `(true, 0)`
-> `(true, 1)`), `self.key` changes and `now.0` is `true`, so the method runs `self.read(cx)` without clearing
`self.last_discovered`, `self.discovery_error`, or `self.checked_at` — clearing only happens in the `else` (`!now.0`,
key cleared) branch (`lines 166-173`). As a result: (1) while the replacement key's read is in flight, sandboxes
discovered under the previous key remain rendered and clickable for Delete; (2) if discovery under the replacement key
returns `OrphanDiscovery::Failed` (such as an auth failure for a mistyped/revoked key or a provider outage) or a
transport error, `Sandboxes::answered` (`lines 242-249`) preserves `self.last_discovered` from the previous key and
draws "Could not refresh sandbox list: ..." above the old key's sandboxes. This contrasts with `Sandboxes::read`
(`lines 21-23, 213`), which explicitly rejects replies when `list.key != key` because a key change invalidates prior-key
evidence (`tensorlake_key.rs:116-118` and `tensorlake_sandbox_tests.rs:92-93` likewise note that a replaced key
invalidates the previous key's list). How to verify: in `tensorlake_sandbox_tests.rs`, save a key, reply with a
discovered sandbox `sb-1`, save a replacement key, and reply to the second `/api/tensorlake/orphans` read with
`failed_discovery("authentication failed", &[])`; `sb-1` from the first key remains painted beneath the auth error. Fix:
in `Sandboxes::follow_account`, when `self.key` transitions from one saved key to another (any change where
`self.key.is_some()` and `now.0` is true, or unconditionally whenever `self.key` changes before `if now.0`), clear
`self.last_discovered`, `self.discovery_error`, and `self.checked_at` (keep `self.cleanups` or let the new read replace
it, since explicit cleanup state is helm-wide).

Reviewer metadata (g16_tensorlake_leftovers_discovery/l5): suggested bucket: other — stale/misleading display of the old
key's discovered sandboxes after replacing a key when discovery is slow or fails. Does not reach `highest`: clicking
Delete still confirms by sandbox name and sends the canonical vendor sandbox ID, which a different Tensorlake
account/project will not resolve (`get_sandbox` returns 404) and a rotated key for the same account resolves to the
exact confirmed sandbox.; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g16_tensorlake_leftovers_discovery/l5`.
