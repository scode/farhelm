# Focused tunnel Allow has an unresolved keyboard-guard gap

Reviewed commit: da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c

## TLDR

The focused Allow tunnels button lacks the repeated/composing-Enter guard used by other confirmations. A supported input
sequence turning that omission into unintended tunnel consent is **unverified after investigation**. The warning starts
on Cancel, and its separate container-Enter action already rejects held and composing input.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation** for unintended consent. The guard asymmetry
is source-confirmed; a security consequence needs the complete input and accepted-consent route before it warrants
`highest`. This is adjacent context reviewed with the dialog framing at `53c8bd6bf2079f683de0f499de15888372e61685`, not
a claim that framing introduced the omission.

At the reviewed tip, `crates/farhelm-ui/src/tunnels.rs:473` chooses Cancel for initial focus and Allow only for the
container's main action. The Allow handler at `515–533` has no keyboard guard. `src/modal_dialog.rs:955–960` forwards an
optional key handler, but this caller supplies none. The shared `guard_confirmation_enter` at `src/lib.rs:225–230`
prevents native activation for repeated or composing Enter; TunnelControls does not use it.

The investigation distinguished two paths. The container listener (`src/dialog_focus.rs:45–49`) requires the event
target to be the container and rejects repeat/composition. Its default Div root has no implicit form-submit fallback
(`src/modal_dialog.rs:882–886,918,1009–1011`). Initial Cancel focus also refutes the proposed automatic Allow through
the ordinary held opener. Those facts do not guard a button that already has focus. The Tab trap at `621–658` moves
focus but does not suppress key repeat. `assets/composition-guard.js:46–64` normalizes event flags without preventing
default behavior. The consent controller's saving guard prevents another save after admission; it does not classify the
first click's keyboard origin.

No complete ordinary focus/key history was established that reaches Allow with repeated or composing Enter without an
independent affirmative action, produces a native click, and admits consent. No browser or native reproduction ran. A
supported event trace showing focus, key flags and the resulting consent request would settle that premise. Source
inspection of the handlers alone establishes neither that route nor its impossibility.

Possible cover is ambiguous: `SPEC.md:1370–1374` forbids repeated/composing Enter for its listed newly opened
confirmations, but does not explicitly list this Cancel-first tunnel warning. The container rule at `1381–1385` rejects
held/composing Enter, without explicitly settling directly focused buttons. The pre-redraw allowance at `3580–3583`
preserves explicitly named guards, so it does not settle that scope question. The existing token/save and tunnel
disclosure findings concern different mechanisms. No matching Planned item, unlanded sentence-case work, known bug,
queue item or triage decision covers this remainder. Proposed drop: none for the focused-button concern; only the
source-refuted opener/container mechanism is excluded. If the rule applies to this warning, attaching the shared guard
would preserve fresh native Enter/Space while consuming repeated/composing Enter.

Provenance: trailing-review run `2b7a8eaf-c4e9-42ab-be93-1acf29106132`, `a01/f18`. The initial narrow refutation and
same-ID correction retaining this remainder are both preserved. Independent `a02/f23` agrees only with the narrow
opener/container refutation and expressly preserves the focused-button caveat. The parent checked the handler, shared
guard, container listener, composition normalization and specification wording.
