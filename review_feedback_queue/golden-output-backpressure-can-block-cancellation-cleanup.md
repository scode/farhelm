# Blocked parallel log output can prevent cancellation cleanup

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

Blocked parallel render output can delay cancellation cleanup and retain the update lock.

## Details

The source route is a direct supported run with multiple worker groups and a still-open blocking stdout whose reader
stops draining it. scripts/check-gui-goldens.py:170–177 records SIGINT/SIGTERM in a flag. After an individual worker
exits, :202 checks the flag once, then :205–207 flushes stdout and synchronously replays the entire temporary log before
checking cancellation again. Python's shutil.copyfileobj uses repeated blocking writes (shutil.py:196–204). A signal
after the last flag check and before a blocked write can leave the flag set with no code able to reach the worker
cleanup at :211–217. This route does not require system-call restart after a signal. Meanwhile later workers may still
be rendering. In update mode the wrapper owns the capture lock around this code (:221–226) and workers inherit it (:165,
:186–189), so the default publish entry point refuses access while the wrapper is stuck (scripts/gui-goldens.py:160–175,
:394–397). This does not certify incomplete captures. The optional evidence recorder has its own whole-process-group
forced cleanup after a grace period (scripts/record-test-run.py:1237–1246, :1281–1299), which bounds this particular
stuck wrapper when the recorder itself is operating normally. Keep output delivery from blocking cancellation and child
cleanup, for example by delivering retained logs after releasing worker ownership and the capture lease or by making
output replay interruptible.

Confidence: confirmed by inspection, conditional on a blocking stdout whose reader has stopped draining it at the replay
boundary.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `goldenRunner/l12`.
