# A shutdown socket setup error bypasses host cleanup and staged-fixture preservation

Reviewed commit: c9660a5b670f8528cdd4ba9674b71270ffdc192b

## TLDR

A shutdown socket setup failure can skip cleanup and discard already staged agent screens.

## Details

**other.** Source inspection.

c9660a5b670f8528cdd4ba9674b71270ffdc192b scripts/agent_screen_host.py:152–174,201–255;
scripts/capture-agent-screens.py:310–312,783–800,853–861; scripts/test-capture-agent-screens.py:179–212. Same paths at
pinned 1a53d9d35889d7dde96e31320cc1681dd013737d.

Even after a successful capture drive, a control socket allocation error during close can leave the host unconfirmed,
cost captured frames and skip later harnesses. This is the transport error boundary: socket allocation occurs outside
the OSError translation, HostRun.stop catches HostError only and never reaches its directory-scoped fallback, and
capture_harness lets the raw exception unwind past fixture publication. Normalize ordinary control-socket setup errors
at HostRun.request, or have HostRun.stop classify them and still perform fallback. Only if fallback cannot confirm
cleanup should close return the retained-directory HostError already handled by capture_harness. Catching the raw error
solely in the capture wrapper could preserve fixtures while still skipping host cleanup. This repair does not handle a
drive error already pending on entry to finally (capture-drive-failure-discards-staged). The stand-in close test
exercises a translated close error after a normally returning drive. The reviewer suggested other for maintainer
tooling. Confirmed source route conditional on an OS allocation failure; no incident asserted. ; related
capture-drive-failure-discards-staged is a separate error boundary.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`capture/capture-close-exception-coverage`.
