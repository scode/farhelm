# Upload failure test lacks pipe holder witness

Reviewed commit: ac2cf44d652ff83cde0e54555b3a2cf12ea40073

## TLDR

The failed-upload regression can pass without starting the pipe-holding fixture needed to test the original hang.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location:
`crates/farhelm-helm/src/provisioning/backend.rs:3508-3547 at ac2cf44d652ff83cde0e54555b3a2cf12ea40073`.

Source assessment (read with the later corrections below) (review record g019/g019-f1, event 6):

Reviewed commit: ac2cf44d652ff83cde0e54555b3a2cf12ea40073. TLDR: The automated check for failed host uploads can report
success without exercising the shared-connection condition that used to leave host setup hanging. Details:
HeldUploadLauncher::spawn at backend.rs:3482-3484 starts an escaped `setsid sh` holder in the background and
unconditionally exits the direct shell with 7. a_failed_upload_does_not_wait_on_a_blocked_feeder at 3508-3547 neither
establishes that the holder started and retained the pipes nor proves the feeder became blocked. It reads holder.pid
only for optional cleanup at 3532-3539, and its only semantic assertion at 3543-3547 accepts either generic
payload-transfer or transferring context. If `setsid` fails to execute, the shell still exits 7, its pipes close, the
feeder sees BrokenPipe, and ssh_put returns the accepted transferring error at 1292-1296; this also satisfies the test
against the old unconditional feeder join because no writer is blocked. Establish a holder-ready signal and owned live
fixture before measuring, and assert an observable proving the feeder is blocked on held input before the failure route
is released. The supplied test-authoring checklist explicitly requires fixture premises before dependent measurement and
an observable distinguishing competing mechanisms. Suggested bucket: other (test coverage defect; no production
data-loss or security route). Confidence: confirmed by source control flow; no runtime experiments performed as
prohibited. proposed_drop=false. possible_cover=not searched; parent handles coverage. Caveat: this does not show the
production fix fails, only that its added regression test can pass without testing the claimed condition. Revision
reason: correct exact line references from numbered pinned source and add temporal check. The complete backend diff from
this commit through pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b changes this test only to supply
SshAccessKind::UserConfig to ssh_put; it adds no fixture-premise assertion or stronger oracle, so the issue remains at
the tip.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g019/g019-f1`; supporting
records `g052/c3`. The retained private log preserves earlier wording and its corrections. This record does not claim
runtime reproduction.
