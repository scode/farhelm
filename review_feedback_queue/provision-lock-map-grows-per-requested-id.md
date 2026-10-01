# Host lock map grows for every host id any request names, registered or not

Reviewed commit: de774a1ee8815ce833da77deac593a55d82f7be3

## TLDR

Every host remove, retarget or alias request adds an entry to a lock table that is never cleaned up, even when the host
id doesn't exist and the request answers 404. A buggy or scripted client that walks host ids grows the helm's memory
without bound until a restart.

## Details

Source: whole-codebase review, 2026-09-30, slice helm-hosts.

Reviewer's confidence: confirmed.

Reviewer's bucket suggestion: other.

Possible cover for triage to check: none (host-write-lock-split-on-actor-respawn.md is about the other lock, the
cache-write lock on the actor handle, not this map's growth)

- `ConnectionManager::provision_locks` (`crates/farhelm-helm/src/manager.rs:1003-1013`) is documented as "Entries are
  never removed; the map is bounded by the hosts registered in this process's lifetime."
- The documented bound is false. `host_provision_lock` (2107-2116) and `try_host_provision_lock` (2125-2137) both do
  `locks.entry(host).or_default()` for whatever `HostId` the caller passes, before anything checks that the host exists.
- Callers that take the id straight from the URL path before any existence check: `remove_host_owned` (hosts.rs:781-788,
  `try_host_provision_lock` first, then `remove_ssh_host` answers 404), `set_destination_owned` (hosts.rs:665) and
  `set_alias` (hosts.rs:724), which both take `host_provision_lock` before the store refuses an unknown id.
- Trigger: authenticated requests such as `DELETE /api/hosts/{n}` for many distinct `n` (HostId is i64). Each adds a map
  slot plus an `Arc<tokio::sync::Mutex<()>>` that stay for the life of the process. Needs an authenticated client, so
  this is resource growth, not a security issue.
- Verify: unit test that calls `try_host_provision_lock` for ids 1..N that are not in the registry and then checks
  `provision_locks.len()`.
- Fix shape: create entries only for ids the registry or actor map knows. For example, look the id up in the actor map
  (or the store) first and return a not-found for unknown ids. Or drop the entry in `remove_host_owned` and prune
  entries with no holder and no registry row. Also correct the doc comment.

Rebase note (main at 1ec60cc): 2c6c487 moved the alias handler's body into `set_alias_owned` in the helm's `hosts.rs`,
which takes the provisioning lock at line 738 before the store refuses an unknown id, so the map still grows the same
way.
