# Mouse-reporting drags still activate OSC hyperlinks

Reviewed commit: 7522d172fce0699cd5e85547a0b536eb821bc13d

## TLDR

Dragging across a program’s hyperlink can still open a website or transfer a host file when the program handles mouse
input. A native modifier-drag can also open an admitted downloaded document.

## Details

Suggested bucket: `highest`. Confidence: likely: the event and admission paths are established statically; actual
browser/desktop reproduction with mouse reporting was not run. No execution or credential disclosure is claimed..
Source: dedicated `gpt-6-astra` high adversarial review. Proposed drop: None. Retain for assessment of the unconsented
file-transfer boundary. Any drop for prior coverage requires independent review..

At crates/farhelm-ui/assets/terminal.js:3516–3523, activation is suppressed only when term.getSelection() is nonempty.
With mouse reporting enabled, xterm disables local selection; its Linkifier still activates matching press/release links
without checking pointer movement (assets/vendor/xterm.js:1, bindMouse and Linkifier._handleMouseUp). Thus a
primary-button drag wholly inside a stable OSC span passes this guard. For a previously checked file target,
terminal-files.js:288–295 repeats the same insufficient selection check and starts the transfer. The later copy-notice
handler at terminal.js:5252–5302 neither cancels nor precedes that activation. This is an incomplete fix of existing
behavior, not a newly introduced regression. SPEC.md:1365–1374 and 2273–2277 require an explicit click to authorize the
host-to-viewer transfer. Track the gesture independently of local selection and cover mouse-reporting mode; the new
terminal-links.spec.ts:57–103 exercises only a plain shell with local selection.

Possible cover: TRIAGE_OUTCOMES.md heading selecting-osc-hyperlink-open-it-unintentionally.md describes the same general
consequence but records a completed selection-guard fix; this adds the unhandled mouse-reporting trigger. Current
SPEC.md:1498–1505 preserves file-link behavior while changing web-click ownership, but does not clearly authorize
drag-triggered transfers.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: terminal.js:3587–3594 and
terminal-files.js:333–335 still use selection-only admission. The vendored xterm is unchanged.
cfb54c4c4f7e0c7018b55f802fb1add03ad0a577 changes web-link mouse ownership, explicitly excluding file links; it does not
fix this file-transfer case.

Review coverage: full commit diff read: true. No commit-diff content remains unread. Surrounding modules and
specifications were read selectively, not in full. The rest of the 13,089-line triage ledger was searched rather than
read sequentially. Current queue finding bodies unrelated to this commit were not opened. Minified xterm was inspected
through bounded relevant excerpts, not as a complete file. Two oversized combined outputs were truncated; affected diff
content was reread, and specification conclusions rely on subsequent explicit numbered ranges rather than truncated
search output. No Sol reports or working-copy queue additions were read.

Runtime evidence: None run; findings are static review. Native IME delivery and the mouse-reporting browser gesture
remain unverified at runtime. No files were created, modified or deleted; no fetch, rebase or helper agents.

## Native file opening extends the consequence

Reviewed commit: 1dc0bde65dcb67aa2632fca8e755747263903e96

Suggested bucket: `highest`. Confidence: likely; activation and event forwarding are confirmed statically, but the
native mouse-reporting modifier-drag was not reproduced. Source: separate adversarial trailing-review agent, desktop and
devbuild scope. Proposed drop: none; extend this finding rather than duplicate it. A fresh independent reviewer agreed
that this extension is required for deduplication.

`crates/farhelm-ui/assets/terminal.js:3602–3610` and `terminal-files.js:391–404` retain selection-only drag admission,
but now forward the mouse event and turn the native modifier into `open: true`. A modifier-drag over an admitted
`.md`/`.txt` target can therefore save it and dispatch it to the default application.
`plans/reports/click-to-open-paths.report.md:22–25` acknowledges the remaining mouse-reporting consequence.
`SPEC.md:2679–2687` at pinned tip requires an explicit modifier-click; it does not authorize drag-triggered opening. The
fix and regression should cover both transfer and default-app dispatch. No arbitrary execution or credential disclosure
is established by this extension.

The complete file-opening patch and relevant surrounding activation paths were read. The extension persists at pinned
tip `75af240c09c7ecd5b81a0fcba9e03f204b0be0a7`; native gesture reproduction remains outstanding.
