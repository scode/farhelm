# The per-agent search checks do not verify the trust result's focus target

Reviewed commit: 4949a79e8465ce1f25e0e5c42a1e76b80730ce0b

## TLDR

The Settings search test can pass with a trust result that names the right agent and focuses the wrong control.

## Details

**other.** Source inspection.

4949a79e8465ce1f25e0e5c42a1e76b80730ce0b crates/gui/farhelm-gui/src/settings/search/tests.rs:129–167,483–525;
settings/search.rs:133–148,332–350,543–545,739–788; settings.rs:1003–1012,1064–1102;
1a53d9d35889d7dde96e31320cc1681dd013737d crates/gui/farhelm-gui/src/settings/search/tests.rs:129–176,493–535

A future agent Settings search result can retain the right label and open the right page while losing the handoff to its
trust switch, without the changed search assertions detecting it. The new Antigravity case checks the section, switch
element ID and match for 'agent trust', but never checks the independent Target::Trust payload. The synonym accepts
every Trust target, so a different agent in that payload still satisfies the check. The catalog-to-drawing check clicks
navigation and checks IDs without activating the search result. Both Enter and click carry the result's section and
target separately; the section determines the read, and focus resolves by the payload's agent. On a fresh opening, the
correct page's successful read cannot make another agent's unread switch available. The actual catalog correctly binds
both fields. The ready-switch, top-of-page and deferred-ntfy search tests
(settings/search/tests.rs:169–204,288–322,406–449 at the assigned commit) cover generic search behavior, while the
trust-hint route creates its own target outside the search catalog (settings.rs:307–322). The same oracle gap remains at
the pinned tip. SPEC.md:2499–2503 specifies revealing the control after click or Enter. An independent expected trust
target or an activation assertion that observes focus after the correct read would pin this binding. The reviewer
suggested other, test coverage only. Confidence confirmed from source, test execution excluded. This concerns identity
chosen at activation, separate from late replies across pages and shared accessible labels.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`agent-settings-checks/agent-search-result-target-oracle`.
