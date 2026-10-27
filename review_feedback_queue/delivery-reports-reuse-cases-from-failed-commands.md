# Delivery reports reuse passing cases from failed commands

Reviewed commit: 1b08b87e75aff445ccbedcfd088995bccea1e960

## TLDR

Several delivery reports combine passing cases from a failed command with narrower reruns, despite the repository's rule
that evidence reuse is per command. They disclose the failures; this is a validation-accounting conflict.

## Details

`plans/reports/trash-delete-blocking.report.md:35–45` explicitly reuses 42 cases from a command with 12 failures. At
`94d6904493199ba6e7f91aae95aa1da85d4f9c1c`, `plans/reports/emoji-streaming-corruption.report.md:41–48,93–99` retains
passing cases from its failed broad command. At `11ce27e971f0892fe6d3e1840a1ca8a2b9bb42d4`,
`plans/reports/steel-foundation.report.md:92–102` similarly credits CSS and browser cases from failed commands. Each
revision's `.agents/validation.md:45–52` requires reuse per command and excludes failed runs as pass evidence; the plans
incorporate those rules.

Suggested bucket: `other`. Confidence: confirmed committed-text conflict. No fabricated execution, concealed failure or
product defect is alleged. Successful focused commands retain their own scope and may provide appropriate evidence
without rerunning a whole suite. The revised Steel report names separate passes and removes the earlier explicit reuse;
that current wording alone does not establish another breach. Possible cover: targeted-check permission creates
interpretive tension but no explicit case-level reuse exception; external historical authorization remains unverified.
Provenance: `g045/f1`, `g049/f1`. Private runtime records were not authenticated.
