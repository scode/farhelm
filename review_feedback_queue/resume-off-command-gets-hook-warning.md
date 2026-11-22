# Command launches with Resume off receive misleading silent-hook warnings

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

A command launch with Resume off can get a misleading silent-hook warning that an arriving report does not clear.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d crates/gui/farhelm-gui/src/launcher/request.rs:490-499;
crates/farhelm-helm/src/sessions.rs:2179-2191; crates/farhelm-supervisor/src/service/core.rs:14679-14684;
crates/farhelm-supervisor/src/service/capture.rs:135-165,355-384;
crates/farhelm-supervisor/src/service/notifications.rs:223-235,478-510;
crates/farhelm-supervisor/src/agent_kind/mod.rs:1889-1895

A command launch whose user left Resume off can still get a warning to wait for a Grok or Antigravity report for
Restart. If the report arrives later, the warning stays unresolved because the launch has no resume command. Reviewed
commit 1a53d9d35889d7dde96e31320cc1681dd013737d. The Command form sends the declared agent with resume=None when Resume
is off. The final dispatch asks for persistent setup by kind and the supervisor marks a verified install as hooked
without checking the resume template. The timer sees hook/input/capture and the warning's kind check sees expected
report timing. Neither checks the independent NoResumeCommand offer. The notification then points the user at report
timing and feedback, while resolution requires Resume and IntegrationSnapshot refuses that offer regardless of any later
capture. SPEC.md:3368-3375 and 4140-4146 describe why this command cannot Restart and require its true reason;
:3129-3155 describes the warning's action and consequences. Check for a missing resume command before issuing a
Restart-oriented warning, or give the missing-report observation wording and resolution independent of Restart for that
launch. The reviewer suggested other for misleading guidance and an unresolved notification. Confidence confirmed in
source conditional on successful setup, qualifying input and a report not arriving within the diagnostic window. This is
independent of recovered-terminal warning loss. ; in supplied inventory.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `hook-supervisor/HS-10`.
