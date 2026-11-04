# A stable re-cut removes retained announcements' notes anchor

Reviewed commit: 54943408ee9fe372fc6536948d7f13e5b32ebafc

## TLDR

After an announcement-bearing stable attempt has spent its tag and is re-cut as the next patch version, carried
announcements retain a Release notes link to the old version. The same procedure renames that version's changelog
heading, and the website stops generating its anchor. The link reaches the release-notes page but cannot navigate to the
intended notes.

## Details

Suggested bucket: `other`, for broken navigation. Confidence: confirmed source contract mismatch.
`could_be_highest: false`; no security/data-loss consequence is established. `proposed_drop: null`.

At the reviewed procedure commit, `releasing/.agents/stable.md:133–141` attaches the selected stable announcement to
version X.Y.Z. The failed tagged-attempt route at `:230–241` spends the pushed tag, advances to the next patch attempt,
retains each announcement's original version/id (`:238–240`) and renames the CHANGELOG heading to the new version
(`:240–241`).

The producer introduced by `56b10d95987e25da70ed58a199b2a21852a740fd`,
`crates/farhelm-helm/src/release_announcements.rs:63–82`, includes the retained older entry on the newer build and
composes `https://farhelm.io/docs/release-notes/#vX.Y.Z` from its original version.
`website/scripts/release-notes.mjs:65–79,155–177` rebuilds the page from main's current changelog headings and emits one
version anchor per heading, with no historical-heading or alias input. The old fragment disappears when its heading is
renamed.

Preserved historical copies do not supply that website anchor. `releasing/.agents/changelog.md:25–34` preserves approved
entries while allowing a stable re-cut to extend the previously approved section; it does not require a second
old-version heading. `stable.md:255–257` retains the old release branch/tag and GitHub notes, but
`website/.agents/release-notes.md:7–10` and `website/astro.config.mjs:40–69` describe generation from main's
CHANGELOG.md. `finalize-release-notes.py:2–22` updates a selected GitHub release body, not website fragment aliases. The
recorded website configuration has only the root redirect (`:15–17`); link validation at `:105–114` adds no old-version
alias.

`SPEC.md`, “Announcements,” `:3418–3426`, and `SPEC_impl.md:293–298` require stable release-note destinations. The
ordinary producer/generator agreement on dotted `#vX.Y.Z` anchors does not address a heading later removed by this
supported re-cut route.

The planning record at `4f88e7d6a781de15a965083fb80379c0c7bbfb1b`,
`plans/queue/in-app-announcements.md:101–102,206–209`, also requires carrying the selected stable list and deriving
stable links from each entry's version. No repair appears in the procedure, producer, generator or relevant
specifications through inclusive tip `c5ae8548439cc5bfa33c11ef7ce8da6129d8072a`.

Preserve an old-version anchor alias or another compatible stable destination while retaining required announcement ids,
exact versions and original per-helm content. Storage keeps original content and URLs for already-enqueued ids, so
silently rewriting table text cannot repair links already queued. Duplicating ids to force redisplay or rewriting stored
content is not the proposed fix direction.

Possible cover: no exact matching acceptance, planned repair, known bug, queue item or ledger decision was found in the
required recording sources at the review tip or recording-time main. The cited related concerns have distinct triggers
or consequences unless explicitly grouped here. No full FILTER.md match was established.

Grouped findings: `g038/l1`, `g036/g036-l9` and `g040/g040-l5` identify this same anchor-removal route. `g040-l1`'s
refuted punctuation mismatch, g040-l2's report-only startup wording and the announcement UI admission/binding findings
are separate concerns.

Caveats: the initial compiled table is empty. This route requires a selected stable announcement, an attempt with a
spent tag and a different-version next-patch re-cut. Same-version pre-tag retry does not rename the heading by this
route; RC entries are linkless. No live website/runtime reproduction or publication action was performed. Retaining old
GitHub notes/tag/branch does not imply the website retains its fragment.

Review provenance: automated committed-code trailing review, run `ffa3b94e-5eb1-4569-8eb5-1a52b62110d1`; `g038`, actual
local ID `l1`, latest findings line 9 (detailed website/history reconciliation at line 8); `g036/g036-l9`, latest line
16; `g040/g040-l5`, latest line 12. The shorthand `g038-l1` refers to the original `g038/l1` identity rather than a new
ID. Primary reviewed procedure commit is the full hash above; producer and planning commits are preserved separately.
