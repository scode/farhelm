# SSH command copies include hidden terminal controls without warning

Reviewed commit: ec767b4912bc238ad20a165148adc6d0d061dd1a

## TLDR

SSH help displays hidden characters as escapes but copies their active bytes without warning. Pasting can therefore do
something different from the command you saw.

## Details

Suggested bucket: `highest`, conservatively for possible unintended terminal input or command execution. Confidence:
likely with an unverified premise: exact raw copying is confirmed; the effect depends on the receiving terminal and
shell, including bracketed-paste handling. No execution reproduction was performed. Source: separate adversarial
trailing-review agent, welcome/SSH scope. Proposed drop: none.

`crates/farhelm-ui/src/ssh_help.rs:102–110` escapes the command for display but copies the original string directly.
`crates/farhelm-helm/src/store.rs:1566–1567` permits controls other than NUL in destinations, and
`crates/farhelm-helm/src/ssh.rs:366–374` preserves them inside shell quotes. Shell quoting protects shell parsing, not
necessarily the terminal editor that receives pasted bytes. The session-header copy path at
`crates/farhelm-ui/src/session_view.rs:3512–3523` already warns about hidden characters; these new buttons bypass that
warning.

The trigger needs a stored destination or state path containing controls and a receiving terminal that interprets those
controls as input actions. Reject such values for manual command composition or disclose the difference before copying.
Verification must inspect clipboard bytes and the receiving terminal's behavior separately from the escaped dialog text.

Possible cover: `SPEC_impl.md:1350–1351` requires exact copying without explicitly accepting undisclosed input effects.
Same-account trust in `SPEC.md:2567–2579` may affect security classification; it does not settle accidental misuse. Text
originates in authenticated operator input, local configuration or `--ensure-hosts`, with no identified
remote-supervisor mutation route. The removed SSH plan report also noted this concern. No exact Planned, accepted bug,
queue, ledger or filter coverage was found; preserve the unverified exploit premise for triage.

Additional provenance: both the landing-report review at `25ddfeb0a6240b2c7b55837f75bc8357b6fdeb11` and the report's
approval deletion at `8d463e232f87f05becd7ed794fd31e76007ecf32` surfaced this concern. Their references are
`plans/reports/ssh-2fa-help.report.md:124–130` at the landing commit and the approval's parent. These are consolidated
observations, not separate product defects introduced by report deletion.
