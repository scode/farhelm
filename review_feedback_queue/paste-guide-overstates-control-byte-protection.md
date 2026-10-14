# Paste guide promises broader protection than the implementation provides

Reviewed commit: 30b94e0a2bea4fcc9b764ed1a6e10f82ed708662

## TLDR

Users are told pasting cannot send unseen keystrokes, although pasted control bytes can still interrupt a running
process.

## Details

Suggested bucket: `highest`. Confidence: likely: implementation mismatch confirmed; harmful reader reliance and an
end-to-end clipboard/process reproduction remain unverified. Source: dedicated `gpt-6.1-sol` high adversarial review.
Proposed drop: none.

website/src/content/docs/docs/using/work-in-a-session.mdx:41 newly says hidden control characters are removed and unseen
keystrokes cannot be typed. At this revision, crates/farhelm-ui/assets/term-bytes.js:48–57 removes only ESC and embedded
ESC[201~ markers; Ctrl-C, Ctrl-D and other control bytes survive. terminal.js:2014–2017,2423–2447 passes sanitized text
to xterm.paste. The vendored xterm.js:1 clipboard module 7861 adds framing only when bracketed-paste mode is enabled;
otherwise it forwards the remaining text directly. terminal.js:4932–4963 and tmux/input.rs:284–306 preserve those bytes.
Trigger: paste clipboard text containing an unseen Ctrl-C into a terminal running a foreground process with normal
signal handling and no paste framing. Consequence: the process can receive SIGINT despite the documented reassurance.
This is a new documentation defect, not a newly introduced input-path vulnerability. Narrow the claim to the actual
escape/framing protection.

Possible cover: TRIAGE_OUTCOMES.md heading embedded-paste-terminators-allow-command-execution-definite.md, lines
7722–7741, deliberately required removal of the embedded paste terminator and permitted broader ESC stripping. It does
not accept a promise that all hidden control characters are removed. SPEC.md:1379–1380 accepts intentional clipboard
delivery, not this safety reassurance.

Current-main status: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: guide line 43 retains the claim, and
term-bytes.js:55–56 retains the same sanitizer. No fixing commit found.

Review coverage: full commit diff and relevant code paths read. Runtime behavior and harmful reliance were not
reproduced; the note preserves the reviewer's stated unverified premises.
