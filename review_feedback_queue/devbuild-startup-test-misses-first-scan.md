# The devbuild startup test misses the first update scan

Reviewed commit: 702e7db538e13b56124c9b6cf18477eee1159f0b

## TLDR

The startup test can pass if a devbuild starts its first automatic update scan before saving the opt-out. It also starts
from a database that has already migrated.

## Details

`crates/farhelm-helm/src/lib.rs:1888–1944` checks the stored setting after the helm exits and checks readiness
separately. The fixture has no eligible remote host and never observes first-scan admission. Its initial Store open also
performs migrations before the startup call. Keep the current persistence cases and add an observation at the first scan
and at the startup migration boundary. This is a source-confirmed test gap. The current source orders the seed before
the controller starts.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `devbuild-policy/f1`.
