# Changelog edits can leave the running docs preview stale

Reviewed commit: 73daeb677934a3f0c8f7a45fb4c1b1d32bea7714

## TLDR

Editing release notes while the docs preview runs can leave reviewers seeing the previous text until the server
restarts. Production builds regenerate correctly.

## Details

Suggested bucket: `other`. Confidence: likely — source tracing supports the missing watch; an actual preview edit was
not reproduced, so incidental watching by another dependency remains unverified. Source: dedicated `gpt-6-astra` high
adversarial review. Proposed drop: none.

website/astro.config.mjs:47–49 registers changelogPath through Astro's addWatchFile, but
website/scripts/release-notes.mjs:44 locates that file outside the website root. In pinned Astro 7.3.5,
integrations/hooks.js:190–191 only adds this path to a list used to recognize restart events;
core/dev/restart.js:169–179 subscribes to existing watcher events without adding those paths to the watcher. Pinned Vite
8.3.1 watches the website root, config dependencies, environment files and public directory
(dist/node/chunks/node.js:24652–24657). CHANGELOG.md is read through readFileSync, not imported, and no other website
code registers it. Thus an ordinary changelog edit has no apparent source of a filesystem event. Explicitly add the
changelog to the server watcher while retaining its restart registration.

Possible cover: None found in the pinned queue index, Planned bucket, BUGS.md or relevant ledger searches. The
rare-glitch filter does not fit ordinary edits whose stale display requires regeneration or restart.

Status at frozen endpoint: Appears to persist at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: astro.config.mjs:47–49 is
unchanged. Its pinned Astro 7.3.8 and Vite 8.3.4 retain the same registration and watcher behavior. No fixing commit
identified.

Review coverage: full commit diff read: true. No commit diff unread. The 13,089-line triage ledger was searched for
relevant coverage rather than read end to end; unrelated queue finding files were not opened. Specifications were read
in relevant excerpts, not in full. Unchanged changelog body lines 111–346 were not completely read; all
heading/link/fence/angle-bracket search results were inspected. An aggregated output was truncated; required excerpts
were reopened individually before use. No changed tests or fixtures exist in this commit.

Runtime evidence: None. No website build, browser run or reproduction test executed. Dependency implementation was
inspected statically. No checkout files were created, modified or deleted; no agents spawned, fetches or rebases
performed.

## Additional affected scope

Reviewed commit: ce66df247ff4ab5a5c2c0f7a75e67bfeab30977c

## TLDR

The maintainer can review stale screenshots and alt text after editing a release’s screenshot list.

## Details

Suggested bucket: `other`. Confidence: likely; the missing dependency/watch registration is confirmed by inspection, but
Astro’s live behavior was not reproduced. Source: dedicated `gpt-6-astra` high adversarial review. Proposed drop: none.

website/scripts/release-notes.mjs:102-106 reads the new JSON files synchronously and embeds their contents into
generated MDX at lines 136-137. Unlike the former MDX imports, these files are neither imported dependencies nor
registered watch inputs: website/astro.config.mjs:47-49 regenerates during configuration setup and watches only
CHANGELOG.md. Editing, adding or deleting a release screenshot list while the preview runs therefore leaves the
generated page unchanged. Re-running the documented preview command also returns immediately when already serving
(website/scripts/preview.sh:163-167). This contradicts website/AGENTS.md:36-42’s saved-edit preview contract. Register
the screenshot directory/files as generation inputs, including additions and removals. Scope: local documentation
review; a fresh production build regenerates correctly.

Possible cover: None found in the pinned queue index, relevant triage decisions, Planned bucket, BUGS.md or filters.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: release-notes.mjs is unchanged and
astro.config.mjs:47-49 still watches only CHANGELOG.md.

Review coverage: full commit diff read: true. No commit diff hunks unread; truncated diff output was reread. Historical
PNG pixels and rendered pages were not inspected. The entire root specifications, changelog and large triage ledger were
not read end to end; relevant sections/search matches were used. Truncated unrelated contextual output was not relied
upon. No Sol reports or new working-copy queue additions were read.

Runtime evidence: None, as requested. Applied the supplied test-authoring checklist to the added capture tests and
changed publisher fixtures by inspection. No checkout files were created, modified or deleted; no fetch or runtime
reproduction.
