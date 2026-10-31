# Container creation inherits authenticated host proxy settings

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Container creation can expose host proxy credentials to the contained workload without an explicit credential-sharing
choice.

## Details

Suggested bucket: **highest**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The matching Podman create path implicitly imports host proxy environment, including authenticated proxy URLs; the
ordinary later exec path can deliver the configured container environment to the agent. Independent source review
confirmed the conditional receiver route. The claim requires an authenticated proxy variable in the creating host
environment. Whether intentional host proxy configuration also authorizes credential export into this isolation boundary
remains ambiguous and is preserved as possible coverage. Automatic export of unrelated helper credentials was
independently refuted and is outside this item. No credential values, payloads or runtime probes were used.

Source locations: `crates/farhelm-supervisor/src/container.rs:130-145,338-380`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g114/f4`, `a016/f4`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
