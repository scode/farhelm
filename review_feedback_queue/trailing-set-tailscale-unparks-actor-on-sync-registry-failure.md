# Toggling Tailscale mode unparks the host actor when registry sync fails after commit

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

If toggling Tailscale mode on an SSH host saves the new setting but immediately hits a local database read error while
refreshing connections, Farhelm unparks the host anyway and can reconnect using the previous SSH verification mode (for
example, ordinary SSH known-hosts instead of Tailscale-verified keys). Retrying the toggle then skips retiring any
shared SSH connections opened during that window.

## Details

**highest.** Source inspection. Main reference `crates/farhelm-helm/src/hosts.rs:1024-1042`.

In `crates/farhelm-helm/src/hosts.rs:1024-1042`:

1. `set_tailscale` commits the new `tailscale` bit via
   `state.podman_places.set_ssh_tailscale(&state.store, &old, spec.tailscale).await` (line 1024-1027) and deletes old
   Tailscale keys (line 1029).
2. It then calls `state.manager.sync_registry().await` (line 1032). If that call fails (e.g., transient SQLite read
   error in `self.store.list_hosts()`), `handle.row` in `ConnectionManager` still holds the old `tailscale` value.
3. Line 1037 unconditionally calls `state.manager.release_hold(host, true)` BEFORE returning `reconciled?` at line 1038.
4. Unparking nudges `HostActor::run` (`crates/farhelm-helm/src/manager.rs:4185-4192`), which calls
   `self.reload_row().await`. On a store read error, `reload_row()` returns `RowStatus::Unknown` and keeps the actor's
   old `row` (`tailscale = !spec.tailscale`), then proceeds to `connect_phase` and dials with the old `SshAccessKind`
   (for a `false -> true` toggle, dialing with `UserConfig` instead of `Tailscale` and opening a fresh `UserConfig`
   ControlMaster).
5. Furthermore, when the user retries `POST /api/hosts/{id}/tailscale` with the desired mode as intended by the comment
   at `hosts.rs:1040` (`// A retry can finish reconciliation of a mode the first request saved.`),
   `old.tailscale == spec.tailscale` takes the `else` branch (`hosts.rs:1039-1042`) and calls
   `state.manager.sync_registry().await?` without taking `ssh_route_lock`, without parking the actor
   (`hold_connection`), and without calling `release_shared_connections` on the old-mode masters that may have been
   opened after step 3. Fix direction: Do not release the connection hold onto an unreconciled old row when
   `set_ssh_tailscale` committed but `sync_registry()` failed (or fail closed inside `HostActor::run` when
   `reload_row()` returns `RowStatus::Unknown` after a hold/reconfigure), and ensure the retry branch
   (`old.tailscale == spec.tailscale`) still checks whether the live actor is on a different mode and, if so, parks and
   retires old masters before reconciling and releasing the hold.

Reviewer metadata (g44-tailscale-hosts-transport/l3): suggested bucket: highest (could be in highest because committing
`tailscale: true` and then unparking the actor while `sync_registry` and `reload_row` fail causes the actor to dial with
the pre-edit `UserConfig` trust policy instead of Tailscale-pinned host keys; note the trigger requires a store read
error immediately after a successful store commit, and `set_tailscale` does return an error response to the caller
stating reconciliation failed).; confidence: confirmed.; possible cover: none..

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g44-tailscale-hosts-transport/l3`.
