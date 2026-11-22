# Per-agent Settings tests leave cross-page trust completion untested

Reviewed commit: 4949a79e8465ce1f25e0e5c42a1e76b80730ce0b

## TLDR

The per-agent Settings tests can miss a trust reply being applied to the page opened after the request.

## Details

**other.** Source inspection.

4949a79e8465ce1f25e0e5c42a1e76b80730ce0b crates/gui/farhelm-gui/src/settings/tests.rs:2178–2217,2252–2302,2324–2345;
crates/gui/farhelm-gui/src/settings.rs:875–880,960–983,1110–1133,1483–1503

A future change could let one agent's pending trust reply settle against the Settings page now selected, showing a
different agent's value or error, without the changed integration assertions detecting that mix-up. This is a coverage
gap; no shipped cross-agent state change is evidenced. The per-page test answers Claude and Codex before navigating away
from either, and the save test closes Settings after settling Codex before exercising Antigravity's refusal. The
close/reopen test checks dialog lifetime with the same agent, and the focus tests move focus while keeping the same
page. The general section-switch test (settings/tests.rs:1247–1299) holds a pushed Sessions preference save across
Hosts, using a different state path from the indexed per-agent Configured trust state. The product actually binds the
read callback and save settlement to the initiating agent's captured slot, independent of the page shown, and clears the
focus handover on navigation. SPEC.md:2481–2484 requires section switches to preserve pending saves and outcomes. A
single-opening overlap across two non-Pi trust pages would pin the initiating agent's result while a different page is
shown and clarify what each page displays on return. The same coverage gap is visible at pinned tip
1a53d9d35889d7dde96e31320cc1681dd013737d (settings/tests.rs:2187–2225,2306–2355,2378–2398). The reviewer suggested other
because only the test gap is established; the backend request remains targeted by its captured path. Confidence
confirmed by source.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`agent-settings-checks/trust-page-inflight-binding`.
