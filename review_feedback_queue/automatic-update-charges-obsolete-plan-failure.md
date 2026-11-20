# An old planning failure can suppress an update after retargeting

Reviewed commit: 076932457a71c98fdbc187347b6bef668c231df7

## TLDR

A host can miss automatic updates after you change its destination if the earlier destination’s update check fails just
as the new host becomes available.

## Details

Update planning captures a destination in `crates/farhelm-helm/src/provisioning/service.rs:1640–1650`. An edit can
publish a new connection incarnation for the same host row while that probe is pending, as described in
`crates/farhelm-helm/src/manager.rs:2029–2082`. The error path in
`crates/farhelm-helm/src/provisioning/automatic.rs:158–165` attributes the old failure to the current incarnation.
Successful stale plans already pass through an incarnation check. Use the captured target configuration when settling a
failure as well. The source establishes the attribution gap and a concurrent edit route. The exact timing was not
measured.

Suggested bucket: high. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `automatic-updates/l9`.
