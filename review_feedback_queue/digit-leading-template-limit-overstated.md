# Digit-leading template-name restriction is overstated

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The stated inability to type digit-leading template names is false on shifted-digit keyboard layouts.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

French XKB Shift+Digit1 yields the character 1, which the inspected classifier treats as Character and Rust appends.
That source route disproves the categorical wording. It does not authenticate macOS native input behavior or require
changing the deliberate chord classifier.

Source locations:
`SPEC.md:915-916; website/src/content/docs/docs/using/launch-templates.mdx:135-136; plans/reports/typed-quick-launch.report.md:28-29`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed d6645f771bee8ed03a66486ef8755834937d759d, second separate discovery pass. TLDR: users are told a template name
beginning with a digit needs a number binding or the launcher, although shifted printable digits can start a typed
sequence word. Concrete route: after arming and releasing the prefix, Shift alone preserves the sequence; a physical
Digit1 with key=1, shiftKey=true, no Ctrl/Meta/Alt is neither bare nor the configured number chord and becomes Character
(assets/template-shortcuts.js:48-54,81-97). Rust appends Character even to an empty word
(src/launch_sequence.rs:652-660), and current typed-name resolution imposes no first-character restriction (:589-605).
Numeric names are valid under the new whitespace and existing shape rules (proto launcher.rs:442-451,480-499).
Accessible ordinary producer: installed /usr/share/X11/xkb/symbols/fr:1-43 defines standard French, with <AE01>
ampersand at the base level and 1 at the shifted level (:11); the other physical number keys likewise have shifted
digits (:12-20). The Linux web client is supported (SPEC.md:77-79). Existing JS test :132-135 confirms the Shift-only
physical-digit printable path with Digit1/!, so the code intentionally distinguishes printed text from bare physical
binding identity. This source/contract counterexample needs no claimed native Mac dispatch or executed keyboard
experiment; none was performed. The report :28-29 repeats the same overbroad deduction from leading bare digits, and the
guide :135 also omits the bare qualification. A narrow repair is to qualify the stated residual to bare physical number
input while preserving Shift-printable behavior; no code change is demanded by this report. Suggested bucket other:
inaccurate stated availability, no harmful launch, security or user-work loss. Confidence confirmed source event-class
route and concrete standard layout mapping; no runtime result claimed. proposed_drop none. possible_cover:
plans/queue/typed-quick-launch.md:29-32 expressly asks for the restriction, but :26-28 also requires printable Shift
text, and :29 reserves bare digits/chords; the plan therefore preserves the same contradictory deduction rather than
explicitly accepting this layout exception or authorizing inaccurate wording. Searches across SPEC.md/SPEC_impl.md,
empty TODO Planned, plans/queue, BUGS.md, full earlier index plus targeted index/ledger searches found no matching fix
or accepted exception; the only digit-leading ledger hit concerns terminal file extensions. No established rare-trigger
filter applies to ordinary French-layout typing. This remains a bounded contract/documentation inconsistency rather than
a broader number-shortcut or keyboard-layout failure.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g153/f7`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
