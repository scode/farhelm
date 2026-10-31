# Wrapper guide Stop advice omits the container exception

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The wrapper guide promises Stop leaves independent tabs running, but built-in container Stop terminates their work too.

## Details

Suggested bucket: **highest**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The guide explicitly includes container context while unconditionally promising independent-tab preservation. In the
built-in container route, Stop ends the whole container and its independent tab processes. An operator relying on that
promise can stop work they expected to preserve; this is distinct from the agent approval card's narrower description of
the same destructive scope. Independent source review retains the documentation consequence as highest. The
configured-wrapper-versus-built-in-container context remains explicit rather than a claim about every wrapper. No Stop
or work-loss reproduction was performed.

Source locations: `website/src/content/docs/docs/agents/agent-wrappers.md:162-163`. Historical revision qualifiers in
those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g089/f1`, `a052/g089-f1`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
