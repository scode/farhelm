# Failed tooling stop deletes the remaining recovery target

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

When the real-agent inspection/capture tool cannot confirm its terminal host stopped, it hides that failure and still
removes the run directory needed to retry the supported stop command. Continued agent execution after the deletion has
not been demonstrated.

## Details

Source coordinates from the original records: `scripts/agent_screen_host.py:223-231`;
`scripts/agent_screen_host.py:198-231`;
`scripts/capture-agent-screens.py:720-725; scripts/agent_screen_host.py:223-231`. References below use these paths'
basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

Exact introducing commit verified: 2d5859c5f144e0c286a0576976aae2d1c66ddc7c. git log --diff-filter=A identifies it, and
the parent-to-commit diff adds the whole scripts/agent_screen_host.py file, including the unchecked subprocess result at
223-228, swallowed exceptions at 229-230, and unconditional rmtree attempt at 231. Earlier inspection established no
later modification before pinned tip. The assigned internal CLI exposure and this backend entered together to capture
the same emulator/environment that production terminal hosts use; the issue is cleanup on failure, not the purpose or
availability of that CLI.

Confirmed source consequence: when fallback stop fails or times out, the caller discards its status/diagnostic and
attempts to delete the existing run directory anyway. When that deletion succeeds it loses the socket and metadata
needed to retry the documented run-directory recovery. hosts/stop.rs:23-26 says an unlinked socket is not rediscovered;
52-58 requires callers to distinguish unconfirmed cleanup so they can retry; HostOutcome::Unconfirmed at 179-183 means a
host may still run. agent_screen.rs:46-52 documents internal stop-terminal-hosts --state-dir as recovery. Failure does
not itself prove that a host or vendor agent is still alive: the host may already have ended without being confirmed. No
particular surviving-agent lifetime, ongoing vendor spend, or persistent orphan process is established. This explicitly
narrows the earlier consequence language while retaining the finding. A fix should retain state and diagnostics and
surface failure until fallback stop returns 0. A focused injected failure test should assert retained run state and
surfaced failure; none was run under this read-only charter.

Ownership: g012 retains l3; commissioner routes it to g047/g048, which own capture pipeline/consumer changes. g013 owns
only the final Codex working-tool-2 text fixture and has no lifecycle ownership. SPEC_impl.md Testing permits
single-user tooling assumptions and within-run PID use, but no accepted rule requires discarding recovery on failed
cleanup. Suggested bucket other, unchanged: confined to maintainer tooling, no catastrophic/highest route established.
Confidence confirmed for suppressed failure and lost recovery target when rmtree succeeds; actual surviving-agent
lifetime unverified and not part of the confirmed impact. proposed_drop false. possible_cover none.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g012/l3`, `g047/f2`,
`g048/f7`. The retained append-only histories preserve all earlier claims and corrections. Reviewed source references
name their own revisions; the inclusive reviewed tip is the hash above. This review used source inspection only, with no
product runtime experiments or live-install changes.
