# A pending capture error still discards verified staged frames after shutdown handling

Reviewed commit: c9660a5b670f8528cdd4ba9674b71270ffdc192b

## TLDR

A capture controller error can discard previously verified frames even after shutdown handling runs.

## Details

**other.** Source inspection.

c9660a5b670f8528cdd4ba9674b71270ffdc192b
scripts/capture-agent-screens.py:247–250,283–292,334–360,665–673,783–807,853–869;
scripts/agent_screen_host.py:157–174,239–255; scripts/test-capture-agent-screens.py:179–212. Same paths at pinned
1a53d9d35889d7dde96e31320cc1681dd013737d.

When the controller stops answering after capture has staged frames, those frames are normally deleted and later
harnesses are skipped. This is the capture lifecycle boundary: a controller read/write error is already a HostError,
Recorder.attempt catches ScenarioTimeout only, and drive's exception remains pending through the finally block. Normal
shutdown and an unconfirmed shutdown caught by that block both let the pending error unwind past fixture publication. An
unconfirmed shutdown is therefore not required for the data-loss path. Capture known drive/controller failures as a
distinct unsuccessful harness outcome, run close independently, publish already validated frames and aggregate drive and
stop outcomes into final status while allowing the harness loop to continue. Normalizing shutdown transport exceptions
(capture-close-exception-coverage) cannot settle an exception that entered finally from drive. The stand-in exercises
only a normally returning drive. The reviewer suggested other for maintainer tooling. Confirmed source route conditional
on a drive controller failure; no incident asserted. ; related capture-close-exception-coverage is a separate error
boundary.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`capture/capture-drive-failure-discards-staged`.
