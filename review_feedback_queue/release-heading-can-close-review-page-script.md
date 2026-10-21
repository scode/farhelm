# A malformed release heading can inject review-page script

Reviewed commit: 25983ee2438819c0737b4588c2a80c93613c333b

## TLDR

A crafted release heading can insert script into the generated release-review page. The input is curated maintainer
prose; attacker influence, browser execution and consequential exposure remain unverified.

## Details

`releasing/notes-review-page.py:256–258` accepts any `## ` heading; `270–271` puts its first token through `json.dumps`
into the script's `RELEASE` constant (`149–150`), and `297` embeds that script in HTML. A token such as
`</script><script>alert(1)</script>` closes the HTML element before JavaScript string parsing. JSON escaping does not
protect this HTML boundary. The revision value is independently hexadecimal (`123–125`). The new URL/attribute guards
(`73–88`) fix a different boundary. The CLI renders without format validation (`446`). Encode script-text values safely
for HTML or reject malformed release tokens, with a distinguishing serialization fixture if corrected.

Suggested bucket: **other** under the explicit maintainer-tooling rule. Confidence: **confirmed unsafe serialization;
malformed attacker-influenced input and practical impact unverified**. No catastrophic or broader security consequence,
native execution or credential access was established. This path predates the link fix; do not call it an introduced
regression. Proposed drop: malformed documented input/pre-existing scope, independently **ambiguous**, so retained.

Possible cover: `releasing/.agents/changelog.md:45–50` and `.agents/stable.md:58–73` require a version/date heading.
However, the renderer expressly omits format validation (`notes-review-page.py:32–34`). The link-hardening ledger at
final `TRIAGE_OUTCOMES.md:17117–17132` does not accept unsafe heading serialization. No exact Planned, BUGS, queue or
filter match was found. Provenance: `tooling-checks-implementation-1/f1`, fresh `audit-release-heading-serialization`,
trailing review through `fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`. No runtime reproduction was performed.

Original finding IDs: `tooling-checks-contracts-1/f1`, `tooling-checks-implementation-1/f1`,
`audit-release-heading-serialization/f1`.
