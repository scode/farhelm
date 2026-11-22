# The newly retained stop outcome still has no retry override for an unavailable build path

Reviewed commit: 4c18498dd4eab58a550294bf5b3e73852419f9e0

## TLDR

A retained capture cannot use the advertised stop retry when the exact build path disappears, even if a compatible
executable is available.

## Details

**other.** Source inspection.

Base 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e scripts/agent_screen_host.py:83–105,223–231; reviewed
4c18498dd4eab58a550294bf5b3e73852419f9e0 scripts/agent_screen_host.py:83–92,104–114,239–256; pinned
1a53d9d35889d7dde96e31320cc1681dd013737d scripts/agent-tui.py:134–141,266–275,308–314, .agents/real-agent-tui.md:24–35,
crates/farhelm-supervisor/src/agent_screen.rs:43–52, and crates/terminal-host/SPEC.md:149–163,497–526.

If the recorded build path disappears, retrying a retained shutdown cannot use another compatible build through the
documented command. Both the saved executable pathname and using only that pathname for fallback existed at the
exclusive base, which ignored the execution failure and deleted the run directory. 4c18498dd preserves the run directory
on that failure and advertises retry; it does not add a replacement choice. agent-tui.py stop still accepts only the run
directory. The host can outlive its starting executable path. Restoring the pathname or invoking the lower-level stop
command from a compatible build remain possible, so this is a limitation of the newly exposed retry behavior. Allow an
explicitly chosen compatible fallback or document the recovery prerequisite. The reviewer suggested other. Confirmed
conditional source behavior; no incident asserted.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`capture/capture-stop-retry-binary-unavailable`.
