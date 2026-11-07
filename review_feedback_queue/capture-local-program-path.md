# A relative executable override can inspect the wrong build

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

Naming a Farhelm binary in the current directory with --farhelm ./farhelm can fail to launch it or silently run another
Farhelm found on PATH, so screen inspection can use a different build than requested.

## Details

Source coordinates from the original records: `scripts/agent_screen_host.py:102-108`;
`scripts/capture-agent-screens.py:765,779-788,183`. References below use these paths' basenames unless another path or
revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

New separate-discovery finding. Both callers pass a pathlib.Path directly: agent-tui.py:62 converts args.farhelm without
resolve; capture-agent-screens.py:765 parses it as Path and passes it through Terminal.start:183. HostRun.start first
checks farhelm.is_file (102), then records farhelm.resolve as the run's selected binary (105), but launches str(farhelm)
at 108. Python Path removes the dot component in ./farhelm, yielding the slashless string farhelm. Local
standard-library source /usr/lib/python3.12/pathlib.py:388-405 filters dot parts and :430-445 joins them without the
original prefix. subprocess.py:1875-1882 selects the executable itself only when dirname is nonempty; a slashless name
is searched through os.get_exec_path(env). Thus with a local farhelm file and a different farhelm in PATH, the file that
passes validation and is written to the run's metadata is not the one launched. If PATH contains no farhelm, Popen
raises despite the explicitly named file existing. No malicious PATH, process identity race, or live-install
modification is needed or claimed. Subsequent fallback cleanup also uses the resolved local binary, rather than the
actual PATH-launched build, potentially producing a development-command version mismatch; no stranded-process
consequence is asserted from that mismatch.

The public tooling contract permits --farhelm to name another built binary (docs/agent-screen-fixtures.md:15-16 and
agent-tui.py:263); no absolute-only restriction is documented or enforced. The introducing version has these same lines
(2d5859c5f144e0c286a0576976aae2d1c66ddc7c:scripts/agent_screen_host.py:102-108), and it remains unchanged at tip.
Resolve the selected binary once and use that exact absolute path for validation, launch and run metadata. A focused
injected-Popen argument assertion for ./farhelm would verify this without launching an install or vendor process.
Suggested bucket other: maintainer-tool correctness; no actual wrong-build capture or user-data/security loss observed.
Confidence confirmed by caller and standard-library source; no runtime experiment performed. proposed_drop false.
possible_cover none in peer logs read for this pass.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g047/d3`, `g048/f13`.
The retained append-only histories preserve all earlier claims and corrections. Reviewed source references name their
own revisions; the inclusive reviewed tip is the hash above. This review used source inspection only, with no product
runtime experiments or live-install changes.
