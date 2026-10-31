# Unapproved SSH-help copy has an unresolved receiver contract

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

An unapproved agent Add-host draft can copy raw SSH help text; the required warning and harmful downstream paste remain
unestablished.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

The configured-subset exception is explicit and remains independently agreed. The distinct remote draft is not
automatically the configured case. Clipboard JSON/arboard paths copy rather than execute, and no external receiving
application's paste contract or consequential action was established. No payload or paste experiment was developed.

Source locations:
`crates/farhelm-ui/src/ssh_help.rs:88-109; crates/farhelm-ui/src/hosts.rs:5429-5436,5595-5636,5755-5761,5793-5795`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Same independent reviewer a042; preserve original g054/f2 and all earlier revisions. Reviewed full commit
d6645f771bee8ed03a66486ef8755834937d759d. AGREE with proposed_drop solely for raw-copy hidden characters or
receiving-shell quoting concerns caused by the user's configured SSH destination or helm-local state directory/cwd:
exact SPEC.md:4023-4027 and matching ledger 13245-13277. The unapproved agent Add draft is not user configuration: its
Help path precedes the distinct Add/Allow action, and SPEC.md:3097-3107 says opening is not approval. Retain that
remainder as OTHER, unverified after investigation, not as a confirmed security defect. This synthesis inspected the
accessible primary clipboard receiver: UI auth.rs:297-349 JSON-encodes the text to the desktop POST; helm
clipboard.rs:109-181 deserializes one string, bounds/admission-checks it, and passes the same request.text to its sink;
UI desktop.rs:1340-1372 gives that text to arboard.set_text. It neither runs a shell nor chooses a paste target. Browser
fallback already inspected at session_view.rs:3535-3543 calls navigator.clipboard.writeText. SPEC_impl.md:1575-1591 and
2002-2011 describe exact-byte clipboard transfer and read-only command help, not external terminal interpretation. Thus
these primary receivers safely settle that the application copies text rather than executing it; they do not settle the
separate harmful-paste premise. Source checks also established POSIX-family single-quote composition and initial agent
char::is_control rejection, plus escaped proposal/help presentation. Exact missing supported receiver premise: a
particular supported destination terminal and shell, its relevant clipboard-to-input interpretation contract, and an
allowed unapproved proposal which produces unintended execution or other security/user-work loss despite those
protections. No receiver program/version/terminal mode is specified by the help path; SPEC.md:3989-3992 names bash/zsh
for Farhelm agent launches and tabs, not a universal external paste contract. The pinned sources end at clipboard APIs
and do not contain that external receiver implementation or guarantee. Concrete version-matched primary receiver
documentation or authorized observational evidence would settle the premise. No examples, payloads, attack mechanics,
runtime evidence or historical private approvals were developed or authenticated. No filter exclusion. proposed_drop
true for configured-value subset only; false for the remote-draft remainder. possible_cover: SPEC.md:4023-4027,
explicitly limited to user configuration, never remote text. Suggested bucket other; confidence confirmed producer/copy
boundary, harmful receiver consequence unverified after investigation.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `a042/g054/f2`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
