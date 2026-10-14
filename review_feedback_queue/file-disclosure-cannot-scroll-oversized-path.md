# Oversized file disclosures hide the target while admitting downloads

Reviewed commit: a5f5f8213eeb0a1a5161c22cf6260501aca81be9

## TLDR

A long path can conceal the filename and size before the user downloads an agent-controlled file.

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
