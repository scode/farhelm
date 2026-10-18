# The template table omits custom permission flags

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

The default template listing shows `permissions custom` without the permission flags that the template applies. The JSON
listing includes them.

## Details

`crates/farhelm/src/render.rs:161–240` does not render `permission_flags`, although the helm listing projection retains
them (`crates/farhelm-helm/src/agent_requests.rs:2210–2226`; `crates/farhelm-proto/src/lib.rs:2005–2021`) and the CLI's
JSON rendering includes them (`crates/farhelm/src/main.rs:1758–1785`). Render the words with safe quoting that preserves
argument boundaries.

Suggested bucket: **other**. Confidence: **confirmed static producer-to-renderer tracing**.
`farhelm agent templates --json` is a complete workaround. No launch alteration or approval bypass is established; no
runtime tests ran.

Possible cover: none. SPEC.md's Agent-spawned sessions section (`2368–2370`) promises template fields and their values,
except launch/resume command text. That exception does not cover custom permission words. SPEC_impl's custom-permission
contract keeps those words paired with Custom. No matching Planned, BUGS, current queue, ledger or filter coverage was
found.

Proposed drop: none. Provenance: persistent Astra/high review, `cli-entry-and-rendering-1/f1`.
