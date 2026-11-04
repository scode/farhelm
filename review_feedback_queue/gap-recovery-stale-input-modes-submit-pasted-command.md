# Gap recovery can turn a multiline Bash paste into command submission

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

If recovery loses the output that enabled bracketed paste in Bash, a later pasted newline can submit a command without a
separate Enter. The conditional Bash 5.2 route is established; whether the accepted wrong-mode recovery limit also
accepts this consequence is ambiguous.

## Details

Suggested bucket: **highest**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conditional
source route; occurrence and frequency unmeasured

Source locations: 70cd68a03c101493e0adbf00bb27bb8004adfe41:SPEC_impl.md:1892-1899;
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/hosts/terminal.rs:3638-3720;
crates/farhelm-supervisor/src/grid_input.rs:224-251,636-644.

Preserve g013/f1 and the actual official Bash/Readline source checks. Earlier missing shell/configuration/journal-route
premises were resolved. Do not retain stale technical-uncertainty wording, invent a destructive payload, or equate broad
wrong-mode acceptance with an independently agreed whole-finding exclusion. Independent ambiguity requires retention.

SPEC_impl.md:1892-1899 expressly retains pre-gap modes. At the same commit, service/grid_stream.rs:1074-1085 gets paste
modes from runtime status; grid_input.rs:224-249 conditionally adds brackets, and 637-643 converts LF to CR. The Bash
source checks below establish the conditional unintended-submission consequence, superseding the earlier narrower
wrong-encoding assessment.

If a host-journal gap loses Bash enabling bracketed paste while Farhelm's retained/restored emulator says disabled,
recovery's resize does not repair the mode. A later multiline paste is unframed and its newline can submit a shell
command without a separate Enter. Suggested bucket corrected to highest for the established conditional
unintended-submission route; possibly_highest true. Confidence corrected to confirmed conditional source route for GNU
Bash 5.2 with ordinary Readline bindings. No missing source premise remains for that supported configuration; occurrence
and frequency are unmeasured, not missing behavior. This supersedes my earlier 'complete compatible
shell/configuration/journal-loss history remains unverified' caveat. The conditional history consists of an off-mode
retained snapshot/state, the shell returning to a prompt while no supervisor interprets its enable output, enough
ordinary background output to trim that enable from the bounded journal, recovery retaining the off bit, and then a
multiline paste before a new enable. It requires neither a constructed protocol payload nor hostile clipboard escapes.
Reconciliation checks: read g013's actual turn2-synthesis.reply and full f1 event history. Its Bash source closes the
application-hook part of my earlier uncertainty: GNU Bash 5.2 sig.c:703-713 merely records SIGWINCH;
bashline.c:4784-4837 checks signals/traps without paste-mode refresh; parse.y:1504-1565 calls readline and forwards the
accepted line with a newline to the parser. I read those excerpts directly from the retained official Bash 5.2 release
source. My prior primary-source reads already established lib/readline/rltty.c:597-675 emits enable on preparation,
signals.c:128-149 and terminal.c:392-405 resize/redisplay, display.c:3385-3413 redraws without setup,
emacs_keymap.c:32-48 maps CR to rl_newline, and text.c:1093-1129 accepts the line. Farhelm's tip
hosts/terminal.rs:3638-3720 directly confirms resize-only recovery retaining emulator modes; tip grid_input.rs:224-251
and :636-644, read in synthesis, conditionally omits brackets and converts LF to CR. These sources agree with g013/f1;
no incompatible premise was identified. Independent exclusion review did not agree to drop the finding: SPEC.md at the
tip, Terminal experience:2139-2165 (especially 2152-2157), expressly accepts lost bracketed-paste modes and wrong-mode
pasted input, but whether that also accepts the established command-submission consequence remains ambiguous.

Possible coverage and caveats: that exact paragraph and SPEC_impl.md Recovery and checkpoints. Older
sanitizer/embedded-terminator decisions have different triggers.

Additional reviewed locations: g013/f1: 060913e55ca9b275958bade1ab7c01eaf94d53eb:SPEC.md:2152-2157; g127/f1:
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/hosts/terminal.rs:3642-3662,3684-3720.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g009/f1`, `g013/f1`,
`g127/f1`. Original append-only events, revisions, withdrawals and independent judgments are retained in that run's
catalogs. Locations refer to the named reviewed revisions; the original revision context is
70cd68a03c101493e0adbf00bb27bb8004adfe41; persists at inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was
a source-only review; runtime occurrence and frequency were not measured.
