## What this was about

An SSH sign-in prompt from Setup, Update or Uninstall could leave **Remove** disabled in the desktop. A Remove sent by
another client or the final step of Uninstall could also stall behind a host or Podman check. The decision was to let
removal proceed and cancel the sign-in once the host is gone.

The desktop now offers **Remove** while a check is waiting for an SSH answer. On successful removal Farhelm closes the
question and ends its SSH process. A returning discovery reply or Setup confirmation stays bound to the original host
entry and address. Removing the host or changing its address means a stale check cannot recreate the entry or set up the
former destination.

## Things you should know

Once a provisioning run has started, you can remove the host after it finishes. If a Remove attempt is refused while its
sign-in check is finishing, the desktop keeps the sign-in result and continues the original check after the refusal
arrives. While a check is still waiting at an SSH prompt, Remove opens the existing confirmation.

## Open questions and possible follow-ups

None.

## Pull requests

- [#4714](https://github.com/scode/farhelm/pull/4714/changes) enables Remove during SSH sign-in and ties late Setup work
  to its original host entry. It resolves the matching TODO and review feedback.

## Checks

- Reused 212 passing helm tests from `af1a8a1e`. The subsequent change adjusted only the desktop test's click sequence.
  A rebuilt Farhelm payload, followed by `cargo nextest run --locked -p farhelm-helm --lib`, covered provisioning, SSH
  sign-in, Remove, Tailscale route locking and SSH discovery. The run used four slots with no retries and no runtime
  skips, including local and self-SSH provisioning (recorder `09b08915-e4c3-4149-b8ef-ea1e897b1056`). The two real cases
  also passed in isolation with the rebuilt payload (`a40df9e9-2e94-4f0a-be9d-eacbbedc7271`). The prior run
  (`803f2eb4-8762-49f8-b97e-fe31a12bd783`) had used a debug payload from before the protocol version bump and is
  retained as a failed run.
- Passed on `828777d1` on Linux. `cargo nextest run --locked -p farhelm-gui --lib` covered the Hosts, sign-in and
  operation-store tests (316 tests, four slots, no retries or runtime skips, recorder
  `13245459-a47a-4380-bcb0-f4e15fde031e`). The exact Remove test passed separately
  (`812fd08b-e154-4dc8-b1e2-aa13a1bb0419`) after correcting its fixture to dismiss the sign-in popover without
  cancelling the question. The earlier failure is retained under `63eb9383-e428-410e-b7e2-d751ff9003fa`.
- The recorded four-process-budget `check-gui-goldens.py --update --scene host-menu --scene host-remove-confirmation`
  run produced four Linux PNGs, each an exact match for its reference (`98b335b7-434d-4472-8a6a-5b46ad08217f`). No
  reference publication was needed. The targeted render covers the existing menu and confirmation in both themes, while
  the desktop regression exercises the pending sign-in state.
- Passed on `828777d1`. `cargo clippy --locked -j 4 --all-targets -- -D warnings`,
  `cargo clippy --locked -j 4 -p farhelm --bins -- -D warnings`, `cargo fmt --all -- --check`, `dprint check` on changed
  Markdown, `git diff --check`, `python -B scripts/check-test-sleeps.py` with the isolated interpreter,
  `python3 scripts/check-agents-md.py`, and `python3 releasing/check-changelog.py format`. The instruction checker
  reported one existing archived citation warning. The full runtime suite and docs-shot refresh were skipped because the
  affected helm and desktop paths have focused coverage and the screenshot layout is unchanged.

## Review

The review gate passed. The plan's Sol reviewer found that the desktop did not offer Remove during a pending sign-in and
found races around retargeting, reconciliation and replay. A fresh scope review confirmed the fixes fit the agreed
behavior. Sol verified the corrections, including preservation of a successful sign-in whose replay waits on a Remove
reply. Opus approved the menu and confirmation captures in both themes.
