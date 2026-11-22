# Settings -> Containers omits the loading indicator and keeps a stale refresh error while reloading

Reviewed commit: 7cd71cdacc4ff2be08190fd7375542949efc0461

## TLDR

After a failed refresh in Settings -> Containers, deleting the last remaining listed leftover (or clicking Refresh to
retry) shows no "Reading container leftovers..." status while the helm queries container hosts (up to 20 seconds),
leaving only the old refresh error visible as if the new check had already failed.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/settings/containers.rs:348-358`.

Note on the earlier lead premise: the suspected round-reuse race between an in-flight Refresh GET and a subsequent
Delete is refuted because both choices::read_route (crates/gui/farhelm-gui/src/settings/choices.rs:513 ->
operations::read -> operations::send) and Containers::delete (crates/gui/farhelm-gui/src/settings/containers.rs:338 ->
operations::send) use Lane::Ordered, so the UI channel serializes the earlier GET to completion before starting DELETE.
However, the loading-state bug in crates/gui/farhelm-gui/src/settings/containers.rs is confirmed: Containers tracks only
leftovers: Option<Vec<ContainerLeftoverView>> and leftover_error: Option<HostText> (lines 103-105) with no reading: bool
flag, and render() (lines 512-530) shows "Reading container leftovers…" only when (&self.leftovers,
&self.leftover_error) matches (None, None). Consequently: (1) If a prior Refresh failed (leaving leftover_error =
Some(old_err) beside retained leftovers = Some([row0])) and the user deletes row0, Containers::delete (lines 348-358)
retains out row0, sets section.leftovers = None when rows becomes empty, and starts Self::read_leftovers(section.round,
cx) without clearing section.leftover_error or marking a read in flight. While that follow-up GET /api/container-orphans
waits (up to 20 seconds), render() hits (None, Some(_)) => Vec::new() (line 529), showing only the old error and no
loading status (contradicting what containers_tests.rs:622 checks for the no-prior-error case). (2) Similarly, clicking
Refresh (start_round, lines 174-184) after a failure leaves leftover_error set and shows no reading/checking status
while the refresh is in flight (compare SPEC.md "Container sessions" / "Topology" sandbox-leftovers pattern: "A read
shows Checking…"). Fix direction: track an explicit reading: bool (set true in read_leftovers, cleared when the matching
round replies) and render a reading/checking status row while reading is true (while still keeping cached rows visible
until the reply arrives).

Reviewer metadata (g05-triage-gui-templates-hosts/l1): suggested bucket: other (could NOT be highest: transient UI
feedback gap while a read is in flight; no data loss or security impact); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g05-triage-gui-templates-hosts/l1`.
