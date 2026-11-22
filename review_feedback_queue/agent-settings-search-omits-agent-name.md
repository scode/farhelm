# Reachable Settings search options for different empty agent pages still share one accessible name

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

Settings search can give different agents' pages or hook controls the same accessible name.

## Details

**other.** Source inspection.

crates/gui/farhelm-gui/src/settings.rs:597–638, 1138–1166, 1606–1614, settings/search.rs:87–97, 170–175, 338–379,
493–501, 528–547, 565–569, 673–704, 744–757, 769–812, settings/frame.rs:193–199, 286–312, settings/card.rs:41–53,
components/option_row.rs:86–98, 171–177, settings/agent_trust.rs:112–126, hook_setup.rs:22–40 and
crates/farhelm-proto/src/launch.rs:173–188 at 1a53d9d35889d7dde96e31320cc1681dd013737d. Introduced in
4949a79e8465ce1f25e0e5c42a1e76b80730ce0b.

A screen-reader user navigating the search result options cannot identify differently targeted empty-agent pages from
their option names alone. The catalog includes non-shell launch harnesses and admits agent sections through the
visibility filter. Later commit bb31959c2f22c96bbbb72da2e60eee4a0b288a6b gives Grok hook settings and adds a second kind
of setting to Antigravity, but Cursor, Goose, OMP and OpenCode still get the empty-page row. Matching includes the
section label and generic placeholder text, so a shared text match returns multiple rows. Enter uses the selected entry
and a click captures its section and Target::Top. The listbox has separate agent headings and presentational wrappers.
The option itself has neither agent name nor labelled group. Its section-qualified element ID keeps click identities
distinct and does not enter the accessible name. The separate navigation buttons do expose the agent names and offer
another route to those pages. Add the agent name to the option's accessible name or associate the option with its
section heading. The reviewer suggested other. Confirmed by source under SPEC.md:2474–2475, 2499–2504.
Assistive-technology speech and navigation remain unverified. ,

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`agent-settings/agent-settings-empty-result-label`, with `hook-settings/HS-6`.
