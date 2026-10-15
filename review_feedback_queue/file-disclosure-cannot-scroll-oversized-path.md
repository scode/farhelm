# Oversized file disclosures hide the target while admitting downloads

Reviewed commit: a5f5f8213eeb0a1a5161c22cf6260501aca81be9

## TLDR

A long path can conceal required directory and size information before the user transfers or opens an agent-controlled
file. The current filename-first layout qualifies the original claim about the leaf itself being hidden; see the
reassessment below.

## Details

Suggested bucket: `highest`. Confidence: likely; clipping and admission follow from the code, but oversized-tooltip
rendering and input behavior were not reproduced in Chromium/WebKit. Source: dedicated `gpt-6.1-sol` high adversarial
review. Proposed drop: none.

At crates/farhelm-ui/assets/app.css:1155, the new file tooltip has viewport-bounded height and overflow:auto, but
inherits pointer-events:none from :1151. It has no keyboard focus or other scrolling control. A deeply nested resolved
path in a short window therefore clips the final path components and size; pointer scrolling reaches the underlying
terminal. terminal-files.js:198–208 nevertheless records ready evidence, and :288–295 admits the download without
checking that the disclosure is readable. A short printed relative path or OSC label can resolve to the oversized path.
This violates SPEC.md:1367–1374 and :2274–2278, which make disclosure the authorization safeguard. Keep the whole
disclosure accessible, or withhold admission when it cannot be inspected.

Possible cover: none found; the queue's pinned-host-dialog-chrome-can-hide-content.md concerns host dialogs, not
terminal file disclosure

Current-main status: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: app.css:1249–1253 retains the same pointer
and overflow rules; terminal-files.js:162–183 and :333–340 still admit solely from ready evidence.

The full commit diff and relevant paths were read. No runtime reproduction was run.

## Redesigned disclosure and native opening

Reviewed commit: 1dc0bde65dcb67aa2632fca8e755747263903e96

Suggested bucket: `highest`. Confidence: likely for clipping of the required disclosure; fresh browser rendering and
harmful reliance remain unverified. Source: separate adversarial trailing-review agent, desktop and devbuild scope, with
a fresh independent deduplication review. Proposed drop: none.

The redesigned hover puts the filename first. The historical claim that a long directory necessarily hides the leaf
therefore needs qualification: the full directory, size, and action can still be clipped. `app.css:1249–1253` retains
the noninteractive overflowing tooltip; `terminal-files.js:194–231,283–293` renders the new disclosure without adding a
way to focus or scroll it. Activation at `:391–404` now permits native modifier-clicks to save an admitted `.md` or
`.txt` target and dispatch it to its default app. Preserve that opening consequence when assessing or fixing this
existing disclosure issue; there is no demonstrated arbitrary-execution consequence.

The complete file-opening patch was read and these paths persist at pinned tip
`75af240c09c7ecd5b81a0fcba9e03f204b0be0a7`. No exact new specification acceptance was found; the independent reviewer
agreed to deduplication with this narrower current-layout claim and added consequence.
