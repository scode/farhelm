# Cancelled discovery probes can leave helper processes behind

Reviewed commit: 1ec60cc06baff3442bcdd73760a9ebdc8b08c49c

## TLDR

Cancelling host discovery can leave probe helpers and stderr-reader tasks alive, so repeated abandoned discoveries may
accumulate processes instead of ending cleanly.

## Details

Found by pre-pr-review-swarm run `20261001-1343-1ec60cc-5337` as `F27 / COR-PROBE-CANCEL`, tagged **possible** because
the exact HTTP-cancellation and surviving-helper sequence was not reproduced.

The discovery endpoint at `crates/farhelm-helm/src/provisioning/http.rs:173` directly awaits the probe. The probe
subprocess is isolated into its own process group at `crates/farhelm-helm/src/provisioning/backend.rs:1199`, and
ordinary success/error paths explicitly terminate that group and join stderr. Dropping the request future skips those
paths. Tokio's `kill_on_drop` fallback targets the direct child, not every member of the isolated group, and dropping
the stderr join handle detaches that reader. A surviving helper that inherited stderr can therefore outlive a cancelled
request.

Give request-owned probes cancellation-safe process-group ownership and bounded reader retirement. Keep intentionally
persistent SSH connection masters outside that cleanup target. Confirm the transport-specific surviving-helper case
before choosing the final cleanup design.
