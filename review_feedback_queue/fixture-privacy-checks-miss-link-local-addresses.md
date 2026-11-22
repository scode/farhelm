# Fixture privacy checks miss link-local IPv4 addresses

Reviewed commit: 9a76910186fae0c031f6b37a4dea670b93b5f796

## TLDR

The capture scrubber and committed fixture checks can miss link-local IPv4 endpoints. No actual disclosure was
established.

## Details

**other.** Source inspection.

9a76910186fae0c031f6b37a4dea670b93b5f796 scripts/capture-agent-screens.py:94–140,196–218,331–343;
crates/farhelm-supervisor/src/agent_kind/screen_fixtures.rs:188–208,246–283,358–379. Pinned
1a53d9d35889d7dde96e31320cc1681dd013737d docs/agent-screen-fixtures.md:29–31,55–61;
crates/farhelm-helm/src/browser.rs:117–125,229–253.

The capture scrubber and the separate validator for committed fixtures both recognize several local address categories
and omit link-local IPv4, including when decoding its IPv6-mapped form. The fixture contract covers private endpoints as
well as credentials. The address classification change consequently leaves a supported category outside both defensive
checks. Whether a supported vendor actually leaves that endpoint or usable credentials visible in a saved frame is
unverified. Align the capture and validation address predicates with the documented privacy scope. See
`scripts/capture-agent-screens.py:94–140,196–218`,
`crates/farhelm-supervisor/src/agent_kind/screen_fixtures.rs:188–208,246–283,358–379` at `9a7691018`, and
`docs/agent-screen-fixtures.md:29–31,39–43,55–61` at the pinned tip.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `capture/capture-omits-link-local-ipv4`.
