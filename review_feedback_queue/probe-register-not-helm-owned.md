# A dropped host probe can leave an invisible, never-connected host

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

If the page reloads while a host probe is registering a discovered supervisor, the host can end up registered but
invisible in the hosts list and never connected, and adding the same destination again is refused as a duplicate, until
the helm restarts or another host edit happens.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F20 / COR-PROBE-REGISTER`, tagged **definite**. Anchor and title:
`crates/farhelm-helm/src/provisioning/http.rs:167` — a host probe dropped mid-registration leaves the host registered
with nothing connecting to it.

When the hosts panel probes an ssh destination and finds a working supervisor there, the probe does not just report
back: it registers the host. Registration (`register`, `crates/farhelm-helm/src/provisioning/service.rs:466-566`) has
three steps:

1. Commit the registry row: insert a new host, or rewrite an existing row's remote binary and state-directory paths.
2. Reconcile. Re-read the registry so the helm starts the host's connection actor (the per-host background task that
   holds the supervisor connection), then ask it to dial now.
3. If reconciling fails, roll back a newly inserted row.

The HTTP handler (`probe_host`, `crates/farhelm-helm/src/provisioning/http.rs:167-175`) runs all of this directly on the
request's own task. The web framework (axum) drops that task if the client goes away, for example on a page reload. If
that happens after step 1 and before step 2 finishes, a new host row exists with no actor. The hosts list is built from
the actors (`host_views`, `crates/farhelm-helm/src/hosts.rs:347`), so the host is invisible and never dialed. Adding the
same destination again by hand is refused as a duplicate. If an existing row was rewritten, the running actor never
learns the new paths. And the rollback in step 3 never runs. The state persists until something else re-reads the
registry: a host edit, a helm restart, or another probe. The route comment in `crates/farhelm-helm/src/lib.rs` (around
line 618) says probing is "non-mutating on absence". That is true only when nothing answers.

SPEC_impl.md's "Who owns an accepted action" section exists for exactly this pattern: commit a change, then reconcile
running state with it. The helm already has a helper for it, `run_owned` in `crates/farhelm-helm/src/lib.rs` (around
line 1858), which runs work on a helm-owned task so a disconnect loses only the reply. Host add and destination edits
already use it, and their docs describe this very outcome as the reason. The window here is short, a few database reads,
so this is rare. The suggested fix is to run the probe's registration (or the whole handler body) through `run_owned`
with an owned reference to the helm state. The ssh probe that comes before registration can stay cancellable. Also
correct the route comment to say that discovery registers the host. For a test, drop the probe future after the row
commits, then assert that either the actor exists or the row was rolled back.
