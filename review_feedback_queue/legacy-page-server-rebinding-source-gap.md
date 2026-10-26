# Legacy page server lacks Host and Origin validation

Reviewed commit: 8e0ffc8ba0587ac844f551ee48911e57e78ccba4

## TLDR

The legacy local static server omits Host/Origin validation. Supported browser admission and an actual confidential
served page remain missing, so rebinding impact is unestablished. The instructed decision-page workflows were separately
retired.

## Details

Suggested bucket: **other**. Confidence: Conditional lead: the source or causal premise named below remains
unestablished. No reproduction was performed.

Source location: `scripts/serve-page.ts:36,49-57,65-75`. Locations and revisions in the assessment below identify the
reviewed source; later main is outside this review's pinned range.

Reviewed commit 8e0ffc8ba0587ac844f551ee48911e57e78ccba4. Verdict remains AMBIGUOUS on a whole-finding exclusion.
Synthesis status INCOMPLETE: useful authorized source progress is exhausted without an identified supported
browser/version source establishing DNS-rebinding admission or actual confidential served-page data. No payload was
manufactured and no reproduction was attempted. Previously acquired official @std/http@1.1.4 source confirms
serveDir:654-827 and serveFile:184-339 do not validate authority; wrapper forwards req.url unchecked. Source SHA-256
9347faf0e94732c0255781889f28232d3a3c50728fc09e486e39a8ba3a210f07 matches official version metadata (metadata download
SHA-256 4f1575226ca7c0df1df38b26d50265fed30869b2a63ab867edc5b92492150c8c). Tests at scripts/serve-page.ts:104-170 use a
literal-loopback base and a public constant fixture, check caching/redirect headers and fallback, and do not supply
either missing premise. No test result is claimed. Deno runtime arbitrary-Host admission also remains uninspected.
Pinned-tip call-site search shows legacy standalone/documented tests only for serve-page and todo-picker serve;
specified planning and triage contracts migrated away. Thus historical page-serving source omission is established,
actual browser disclosure is not, and migrated replacement security is outside scope. possible_cover remains none exact
for rebinding: local-hostile-account acceptance, helm authentication rules and consented local tunnels do not match this
remote-page mechanism. No FILTER exclusion. Suggested bucket other; possible catastrophic/broader harm is unsupported
and retained as a missing route, not asserted as highest. Confidence confirmed application/dependency omission,
conditional disclosure with missing source/data premises. proposed_drop: no whole drop; retain conditional other concern
and stop this lead INCOMPLETE. No correction or withdrawal of the source mechanism.

Provenance: `a029/f1`, `g217/f1`. These are reviewer-local identities, not additional findings. Proposed drop: none for
the retained remainder.
