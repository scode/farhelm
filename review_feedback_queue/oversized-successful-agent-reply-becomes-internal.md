# Oversized successful agent reply becomes Internal

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

An oversized success reply could be reported as an internal failure after mutation.

## Details

F326 — **possible** — `crates/farhelm-helm/src/client.rs:1147` — Oversized successful agent reply becomes Internal

The fallback preserves Timeout only for an existing error; an oversized success becomes Internal. Peer-controlled title
and directory fields enter successful session-mutation replies without a local bound at that projection. No concrete
oversized payload establishes reachability, and frame limits may provide enough headroom. Establish the full encoded
bound, then bound those fields or preserve an appropriate post-mutation outcome if fallback is reachable. Retry and
work-loss effects remain hypotheses.

## Evidence and triage context

- client.rs:1149-1177 preserves Timeout only when the input is already an error; any oversized success becomes Internal.
  agent_requests.rs:2193-2242 builds successful mutation replies; 2397-2407 copies peer title and cwd into AgentSession
  without a local size check there. The template path has a concrete shape bound at agent_requests.rs:1739-1740 and
  launcher.rs:336-340, but that does not establish the bound for every session mutation reply.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No exact oversized successful wire payload or ordinary production trigger demonstrated.
- The frame cap and smaller projection may provide sufficient headroom; that arithmetic was not fully established.
- Potential retry/work-loss implications remain hypotheses, not confirmed consequences.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `hc_edges:p1:C10`.

- `hc_edges:p1:C10`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.
