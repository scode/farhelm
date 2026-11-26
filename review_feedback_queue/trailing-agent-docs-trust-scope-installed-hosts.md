# Agent docs pages still scope trust switches to 'installed hosts'

Reviewed commit: 571096872efe7732b4396cd7c3fa46841da0f85a

## TLDR

The commit rewrote the Settings help and SPEC to say the per-agent trust switches apply on local hosts, SSH hosts and
persistent hosts created with Add host (Tensorlake sandbox hosts), because 'installed/recipe hosts' was misleading. The
per-agent docs pages were left saying the switch applies 'on installed hosts', while the docs' own Workspace trust
section lists 'installed hosts and hosts you create with Add host' as two separate groups. A reader of the Codex, Muse,
Pi, Antigravity or Claude page can conclude that turning the switch on does not affect their Add-host Tensorlake
sandbox, when it does: repository hooks, settings and commands there become trusted too.

## Details

Priority bucket: `other`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t04c-settings-save-trust` `f1` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Reviewer's suggested bucket kept. This concerns website prose, which the trailing review does not take as a review
target; it is recorded because it is an incomplete part of an in-scope change (571096872e fixed the same wording in
Settings and SPEC.md). Considered for `highest` as a documentation error touching a trust setting: kept in `other`
because the Settings help shown next to the switch, where the user opts in, is correct, and the docs understate rather
than hide where trust applies on the user's own persistent hosts. No coverage source covers it.

### Reviewer record

Location: `website/src/content/docs/docs/agents/codex.md:29`

At the reviewed commit (unchanged at tip f0c042c9): website/src/content/docs/docs/agents/codex.md:29 ('On installed
hosts, **Answer Codex's folder-trust question** ... skips the question'), muse.md:42-43 ('applies in any directory on
installed hosts'), pi.md:15 ('in any directory on installed hosts'), antigravity.md:18 ('To skip the question on
installed hosts'), claude.md:17 ('On an installed host, Claude may ask').
website/src/content/docs/docs/using/start-a-session.mdx:384 says 'On installed hosts and hosts you create with **Add
host**, trust follows **Settings → Agents**', which reads 'installed host' as excluding Add-host sandboxes. The code
decides by ownership only: crates/farhelm-helm/src/workspace_trust.rs:30-37 (TrustDestination::for_host: session-owned
-> Disposable, everything else -> HostSettings) and :125-128 (HostSettings consults the switch), so an Add-host
Tensorlake host follows the switch. The commit's own SPEC.md:1883 and agent_trust.rs TRUST_LAUNCHES now say exactly
this; the docs pages were not updated. Fix: reword each agent page to 'on hosts that follow Settings (local, SSH and
hosts you create with Add host)' or link to the Workspace trust section's definition.

Suggested bucket: other. It is a scope misdescription of an opt-in switch, and the Settings help itself (the text shown
next to the switch the user turns on) is now correct; it would move up only if a reader relied on the docs page alone to
decide that enabling trust was safe for a sandbox host holding untrusted repositories.

Confidence: confirmed (source-established that the docs wording differs from the code's ownership rule and the commit's
new SPEC text; whether readers take 'installed host' to exclude Add-host sandboxes is interpretation, supported by
start-a-session.mdx listing them separately).

proposed_drop: none

possible_cover: none known; these docs files are outside the commit's snapshots (prose docs), so this is an
incompleteness of the commit's fix rather than a defect it introduced.
