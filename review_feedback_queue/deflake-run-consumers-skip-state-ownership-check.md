# Deflake can reopen run state that its start command would refuse

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

Deflake Stop can trust run state from a location Start would refuse and affect a process outside that sweep.

## Details

The start path validates ownership and privacy before using persistent deflake state
(`deflake/bin/deflake:95–113,1332–1334`). Existing-run commands use a separate resolver
(`deflake/bin/deflake:1321–1329,1421,1461,1491`) that accepts state based on path existence. Stop consults process
identity from those records (`deflake/bin/deflake:643–657,1490–1496`). That identity check handles ordinary process
reuse for authentic records. It cannot establish that the state or recorded process belongs to this sweep when the state
path is untrusted. Apply the same validated-root contract to every reader of persistent run state, contain pointers
within that root, and establish the daemon’s relationship to the sweep before ending it. The documented Stop operation
is limited to its own sweep (`deflake/AGENTS.md:150–156`).

Confidence: confirmed by inspection for a state path controlled by another account and consumed by the documented stop
route. No runtime or abuse investigation.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `releaseGate/l10`.
