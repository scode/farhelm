# The App page heading describes only one of its two scopes

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

The App heading says This Mac even though it contains a helm-shared highlights setting.

## Details

settings/frame.rs:68–88 maps the entire App page to Device and :338–366 renders This Mac as the visible and accessible
scope. The hover on eligible builds mentions pushed changes but does not name the mixed scope. Settings draws the shared
switch in settings.rs:1507–1526. In 4aa99a71ca81c3ad2e687dc906f10415c1ef8bd0 the grouping change added the device scope
to the heading. 7271212a1bcdfe35ffc4df6193412e1debcbcfc5 made the switch description more explicit for both states
(settings/choices.rs:317–326). It left the heading scope intact, as did the rest of the range. SPEC.md:2440–2445
requires the heading to show the page's scope, while :2565–2574 expressly places both scopes in App. The contract
accepts their shared placement and does not prescribe exact badge wording. Suggested mitigation: make the heading
reflect both scopes when the highlights switch is present and keep This Mac on builds with installation-only controls.

Confidence: confirmed by inspection.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `upgradeSettings/l6`, `nav/l1`.
