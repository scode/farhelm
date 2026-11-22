# Launch without loses the one-launch decision across environment admission and late replacement dispatch

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

A launch accepted with Launch without can still install hooks after the user enables setup for future launches while
provisioning continues.

## Details

**high.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d SPEC.md:3085-3096,5491-5513,5533-5557,5572-5577;
SPEC_impl.md:342-359,973-991,4518-4528; crates/gui/farhelm-gui/src/launcher/view/hook_question.rs:63-86,142-179,187-205;
crates/gui/farhelm-gui/src/launcher/draft.rs:653-660;
crates/gui/farhelm-gui/src/launcher/view.rs:1335-1357,1414-1419,1459-1470,1791-1803;
crates/gui/farhelm-gui/src/launcher/view/choices.rs:89-101,247-252;
crates/gui/farhelm-gui/src/session_list/host_rail.rs:890-914;
crates/gui/farhelm-gui/src/settings.rs:1175-1207,1575-1593; crates/farhelm-helm/src/sessions/sandbox.rs:89-121;
crates/farhelm-helm/src/provisioning/tensorlake_add.rs:471-533,1099-1121;
crates/farhelm-helm/src/sessions.rs:1867-1905,1990-2009,2178-2199,3774-3837,4799-4809,4870-4884,5111-5136;
crates/farhelm-helm/src/client.rs:2948-2958,3012-3018,3249-3266; crates/farhelm-helm/src/ui_channel.rs:998-1057;
crates/farhelm-supervisor/src/service/handlers.rs:587-619

Farhelm can attempt persistent hook setup on a launch whose button said Launch without if the user later turns the
all-hosts setting on before dispatch. The same window can make that later Settings save during a new environment's
normal provisioning interval because the launcher hides and the route returns 202 before final create. A Replace with
launch to an existing host can reach the same ordering if its required live session-list read stays pending through the
20-second 504 and then succeeds. The server task survives the timeout, the supervisor client applies no request deadline
to that read, and the user can dismiss a returned launcher and open Settings. A UI channel reset also removes the old
dialog and starts a fresh ordered channel while accepted helm work continues. That lifecycle alone does not establish a
long enough pre-read wait for an ordinary unkeyed New or Clone, tracked in HH-07. No simultaneous windows or CLI writer
is needed for the environment and late replacement paths. This is one missing request field and shared dispatch
decision: the global setting is stored and read correctly, but the one-off skip is discarded before either request is
sent. SPEC.md:3092-3096 explicitly says the answer makes no settings write; its label and the shown-answer rule give it
a one-launch effect. SPEC_impl.md:4520-4523 requires a fresh read of general consent. Carry an explicit skip through the
request and owned work and let the final decision require both current consent and absence of the skip. Do not rewrite
either the opt-in or the separate dismissal. The reviewer suggested other, revised from high to align impact with
hook-supervisor HS-06: the source-supported effect is applying an expressly granted later global opt-in to the wrong
launch, with no established excessive file scope, loss of work, or severe degradation. Setup itself remains conditional
on host policy, timing, and installer success. Consolidate HH-04 and hook-supervisor HS-06.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `hook-helm/HH-04`, with
`hook-helm/HH-07`, `hook-protocol/launch-without-current-opt-in`, `hook-supervisor/HS-06`, `hook-supervisor/HS-07`,
`hook-launcher/HL-8`, `hook-contract/hook-launch-without-stale-settings`.
