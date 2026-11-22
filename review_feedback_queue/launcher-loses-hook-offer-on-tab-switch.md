# Switching tabs can cancel the first hook offer while keeping the explicitly chosen agent

Reviewed commit: bb31959c2f22c96bbbb72da2e60eee4a0b288a6b

## TLDR

Switching tabs during the hook settings read can suppress the setup offer while retaining the chosen agent.

## Details

**other.** Source inspection.

bb31959c2f22c96bbbb72da2e60eee4a0b288a6b and 1a53d9d35889d7dde96e31320cc1681dd013737d
crates/gui/farhelm-gui/src/launcher/view/hook_question.rs:27-32,45-85,94-104,115-123;
launcher/view.rs:961-974,1296-1316,2473-2494; launcher/draft.rs:669-675; 1a53d9d35889d7dde96e31320cc1681dd013737d
SPEC.md:1053-1056,3092-3096

Explicitly choosing Grok or Antigravity, briefly switching to the Command tab with no declared agent, and returning can
leave the selected agent intact while the first-run question never appears. The selection starts a settings read. Retire
runs when the active hook agent changes, drops its pick, and the callback ignores the canceled pick even if the read
later confirms off. The return tab handler calls changed but never starts another check. All check_hook_question call
sites are deliberate harness, declared-agent, or different-model picks, so ordinary launched use of that retained Agent
tab can proceed without an offer. This does not save a dismissal or prevent the later session hint. Suggested other for
suppressing the promised first-pick offer in this narrower UI path. Confirmed state transitions in source. Resume the
pending check or recheck the retained explicit choice if the offer was never presented.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `hook-launcher/HL-12`.
