# An adjacent link-label edit can leave its mismatch warning stale

Reviewed commit: 9d266e28595e8c2c07b378174f35eedf84258a70

## TLDR

A terminal program may change a multiline link's label while the pointer stays over its unchanged first row. Farhelm can
retain the ordinary hover instead of warning that the changed label names a different destination. The correct actual
destination remains visible; rendering and click behavior have not been verified.

## Details

Found by persistent reviewer B12-web-target-disclosure, B12-f1, and independently assessed by static-check-28. Suggested
bucket: provisional highest because this weakens a warning against misleading agent-controlled links. Security
significance remains unsettled because the correct real host is still fully disclosed. Confidence: likely; the source
dependency mismatch is established, but narrow redraw scheduling and persistence through a native click are unverified.
No hidden destination, unauthorized gesture, geometry bypass or exploitation is established.

`crates/farhelm-ui/assets/terminal-links.js:451–515` reads consecutive same-tag rows when comparing a link's label with
its target. `terminal.js:3761–3772` computes that comparison only in the OSC provider's hover callback. The bundled
`vendor/xterm.js:1` OSC provider returns a one-row range. Its Linkifier redraw callback refreshes only when that range
falls inside the rendered interval; a redraw confined to an adjacent contributing row need not refresh Farhelm's
multirow comparison. Mouse activation compares the cached URI and provider range. The click path at
`terminal.js:3659–3660,3672–3682` checks existing disclosure geometry through `terminal-files.js:12–21,380–385`, without
recomputing the joined label.

An unexecuted example starts with `https://` and `first.example/` on consecutive rows sharing one OSC id and target
`https://first.example/`. While the pointer remains over the first row, the program changes only the second to
`second.example/`. If the render interval excludes the first row, the quiet disclosure can persist. Re-hovering or
repainting the hovered row refreshes it. This differs from a mismatched hover/activation URI: the actual URI stays
unchanged.

SPEC.md:1921–1938 requires comparison of consecutive same-link rows and a prominent mismatch warning. Its accepted
unlinked or ambiguously joined fragments do not clearly cover this otherwise joinable edit. Possible cover:
SPEC_impl.md:1031–1042 and TRIAGE_OUTCOMES.md:7680–7706 reserve non-security vendor defects for upgrades. Coverage is
not exact: here the vendor's one-row target remains current while Farhelm adds neighboring-row dependencies. The
repeated-link decision at ledger16915–16935 and refusal-stacking decision at16440–16460 concern different effects. No
exact Planned, BUGS or queued coverage was found. No filter supplies a security exception. Proposed drop: none.

Bind warning validity to every row used in comparison, or recheck that evidence before activation and refuse stale
disclosure. Source inspection only; no runtime check or reproduction was run or requested.
