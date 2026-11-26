# Cutting a stable release

The stable release procedure, step by step, and how a stable release is re-cut after a failed attempt. It is the main
file of the stable release reading list in `releasing/AGENTS.md`, and the fixing forward and stable re-cuts and
changelog curation reading lists include it. Step numbers in this file refer to the numbered steps of
`releasing/.agents/stable.md`, "Cutting a stable release".

> NOTE: In an unattended run (plan executor, landing monitor, delegate), an instruction here to ask, confirm with, or
> propose to the maintainer follows `AGENTS.md`, "Unattended runs".
>
> NOTE: When the release stops to wait on the maintainer at a point where this procedure already waits (a question it
> says to ask, a line or task only they can do, an approval or verdict, a problem you cannot get past), send an urgent
> notification first, per `releasing/AGENTS.md`, "Notifying the maintainer". That adds no stops: what runs without
> asking also runs without a notification.

A stable release is cut from main. The changelog section and any selected announcement additions or changes land on main
FIRST, in the changelog PR; the release branch is cut after it merges, so the bump commit keeps its three-file shape and
main is the only place the notes ever live. The section lands as a draft, and the procedure does not wait for the
maintainer to approve its wording: the tag builds, and the release can be signed and brick tested, while they review it.
The approved text then replaces the draft on main and on the GitHub release (step 3). The tag keeps its original copy.
The installed app gets its upgrade highlights from the website's `/whats-new.json`, which includes only finalized notes.
That lets the build and brick test run during notes review. Promotion in step 8 waits for the website to serve the new
release's highlights. The human-readable Release notes page can still show a draft if the site is deployed during
review.

The version, announcements and any upgrade-floor raise are settled when the stable release is requested, and from then
on nothing waits for the maintainer until the tag's build is running. Apply `releasing/AGENTS.md`, "Rules for every
release", to compare the previous stable's floor with the proposed main base and obtain acceptance of a raise. An
earlier-base choice pauses this default main-based procedure there, before step 1.

Use the version named in the request. If the request omits it, look up the most recent published release, including
prereleases and ordered by publication time (the same lookup as `releasing/.agents/rc.md`, "Cutting an RC release"). If
it is `X.Y.Z-rc.N` or `X.Y.Z-dev.N`, use `X.Y.Z`. If it is stable, increment the minor version and reset the patch to
zero. For example, after stable `v0.40.0`, cut `v0.41.0`.

Check that the chosen version's tag is available, then announce the version in the request-settling reply. Ask which
version to cut in that reply only if neither the request nor the lookup yields a version, or the chosen tag already
exists. The release branch starts at the changelog PR's merge commit (step 5). Settle the version up front so the tag's
build gate can start as soon as the pre-tag checks pass. The v0.29.0 attempt stopped there to confirm a version that
rc.3 had already settled.

In that same request-settling reply, show main's selected announcements for `X.Y.Z` and its prereleases and ask whether
to add or change any, per `releasing/AGENTS.md`, "In-app announcements". Ask even when the version is already settled,
including when there was no earlier RC. "No" is enough, and a request that already says so ("no announcements") has
answered it: show the list as information and go on. This is separate from the release-note wording review that runs
alongside the tag build. Wait for the announcement answer before step 1, and carry any additions or changes in step 4's
changelog PR. Stable re-cuts carry over the settled list without asking again.

1. Fetch tags, then run `python3 releasing/check-changelog.py fragments`. Review every required commit since the last
   stable release. Read the commit and PR when a fragment is `MISSING`. Draft a full-list entry for every user-facing
   change, and pick the ones to propose as highlights (`releasing/.agents/changelog.md`, "Highlights and the full list",
   and "What gets an entry"). `kind: none` fragments are left out of the draft and `STALE` ones are not used (see
   `releasing/.agents/changelog.md`, "The checker"); both are listed for the maintainer, the former as proposed
   exclusions (the fragment author's judgment is not the user's decision), in the message that hands over the review
   page in step 2. Their answers arrive with the wording review and change the draft like any other edit; nothing in
   this step waits for them. This is the sweep the fragment rule exists to make cheap.

   Then run `python3 releasing/check-changelog.py plans`, which lists the plans landed since the last stable release
   with the release note each one settled at planning time. Their entries are the release's highlights, and their
   opt-outs decide which plans get no highlight or are asked about, as `releasing/.agents/changelog.md`, "Plans' release
   notes", describes.

   In the same step, ALWAYS check whether the helm–supervisor protocol version changed since the last stable release,
   and tell the user the answer either way. Compare `PROTOCOL_VERSION` in `crates/farhelm-proto/src/lib.rs` at the merge
   base the sweep uses and at the release base, and list the commits that changed it:

   ```
   base=$(git merge-base vPREV origin/main)
   git log --oneline -G 'PROTOCOL_VERSION: u32 = [0-9]+' "$base"..origin/main -- crates/farhelm-proto/src/lib.rs
   ```

   (`vPREV` is the last stable tag.) Neither the sweep nor the fragments can be trusted to surface this: a bump often
   rides in a `refactor`, which needs no fragment, or inside a feature whose fragment is about something else. A
   protocol change informs release testing. Updating remote hosts is part of an ordinary upgrade, so the change gets no
   entry, Breaking or otherwise, in the notes (`releasing/.agents/changelog.md`, "Format").

   Start the risk report (`releasing/.agents/risk-report.md`, "The release risk report") now too. It must reach the
   maintainer before the tag is pushed in step 7, but the tag does not wait for a reply to it.
2. Read `releasing/EDITORIAL_GUIDANCE.md`, then draft the section from the fragments, the plans' release notes, and
   whatever the user and the agent agree on in conversation, following that guidance and
   `releasing/.agents/changelog.md`, "Highlights and the full list": the plans' entries and the maintainer's own as the
   highlights, and every change in the full list, highlighted or not, with proposed highlights there alone until the
   maintainer promotes them. A plan's entry goes in word for word, and its possible inaccuracies are called out on the
   review page and in the handover message rather than fixed, per `releasing/.agents/changelog.md`, "Plans' release
   notes". Draft a headline for every highlight that needs one, including each proposed addition to the highlights.
   Follow the headline format in "Format". Keep the maintainer's wording in the draft while its headline is awaiting
   approval, whether the wording came from a plan or during curation. Keep the whole proposed `## vX.Y.Z - YYYY-MM-DD`
   section in a Markdown file, the section file, at `farhelm/flows/release/<checkout>/vX.Y.Z-changelog.md` in the
   checkout's attended clone, next to the release log and pushed with it (`.agents/agent-records.md`, "Attended flows'
   logs"), written exactly as it will go into `CHANGELOG.md`, the full list's `<details>` frame included, and show it
   using the `interactive-artifact` skill. Load and follow that skill by name. Do not publish the page as a snippet or
   anywhere else unless the maintainer asks.

   Write a body fragment as `page.html` in a private directory inside your scratch directory (`AGENTS.md`, "Agent
   scratch space"), rendering the section file faithfully with its categories, entries and PR diff links. Show the full
   list as its own part of the page below the highlights, open, so the maintainer can comment on it without unfolding
   anything. Mark every proposed highlight with the reason for proposing it and a picker to promote it, and every
   deferred plan's draft (which sits in the full list alone until the maintainer answers) with a picker to approve it as
   a highlight and a field for their own text. Show each proposed headline as a suggestion on its entry, so the
   maintainer can approve or edit the headline with the rest of the wording. Apply approved suggestions to the section
   file. Offer final approval of the section once every retained highlight has its approved headline or is exempt from
   the headline rule. Do not use the legacy notes-review page generator: it emits a full document with its own scripts,
   which the skill does not accept. The Markdown section file stays authoritative; page markup is only its presentation.
   Follow the skill for styling, serving, sharing the URL and any tunnel instructions, waiting for submissions, status,
   replies and shutdown. Use its chosen port and tokenized URL. If the skill is unavailable or its server fails, stop
   and report why. Give the maintainer the URL as soon as the page is served, and list the proposed highlights in that
   message.

   The maintainer comments on entries, suggests replacements, or submits general notes through the skill. Match feedback
   to the quoted text and section, rather than positional entry ids that shift between rounds. Apply the feedback to the
   section file, update the same page in place, and reply to and resolve handled threads as the skill describes.
   Removing, rewording or merging highlights leaves the full list as it is, and a merged highlight cites every PR the
   highlights it replaces cited (`releasing/.agents/changelog.md`, "Highlights and the full list"). Ask for explicit
   approval of the current text, using a picker whose id changes with each draft revision so approval of an earlier
   draft cannot carry over. An edit request is not approval, and an approval submitted with edits approves only the
   wording those edits unambiguously settle; ask if that is unclear. Iterate until the maintainer approves the text,
   then finish the page using the skill's completion procedure. The section file and the page hold the section as it
   will read once approved, without the draft notice; `CHANGELOG.md` gets the notice added in step 4.

   Do not wait for approval: go on to step 4 as soon as the page is served, and keep the review going alongside the rest
   of the procedure. Step 3 says how the approved text lands.

   Treat any wording the maintainer supplies as authoritative. Reproduce it exactly: do not summarize, paraphrase,
   polish, reorder, or silently correct it. When the maintainer gives edits to the agent's proposed wording, apply
   precisely those edits, without interpreting them as permission for other changes. An explicit request to write or
   rewrite text grants that latitude only for the requested text. Ask when an instruction is ambiguous. Format or
   checker problems must be brought back to the maintainer rather than silently changing reviewed wording. When
   transferring the approved section into `CHANGELOG.md`, copy the section file verbatim, including headings, blank
   lines, and every word; add only the separation needed between the section and the surrounding changelog sections.
   Verify the copy by comparing it against the section file. Do not run a formatter over the approved section.
3. While iterating, watch for feedback that generalizes beyond the entry it was given on: a word the maintainer calls
   internal jargon, a shape of sentence they keep rewriting, a kind of detail they keep cutting or adding. Two ways a
   rule gets into `releasing/EDITORIAL_GUIDANCE.md`, and only two: the maintainer states it as a rule in so many words
   ("X is internal jargon, use Y"), in which case record it; or the agent infers a generalization from the edits, in
   which case ASK, naming the rule it would write, and record it only on a yes. Never write an inferred rule on the
   agent's own judgment; the file is the maintainer's opinions, and a guessed one would steer every future draft. Record
   each rule in the PR that lands the approved text (this step's finalization PR, or the changelog PR in step 4 when
   approval came first), with the example that prompted it. Feedback that only fixes the one entry is applied and not
   recorded.

   When the maintainer approves the text, start or continue the release screenshot subagent immediately, using step 4's
   fixed prompt. Assume the release is going out: prepare the full notes with their app shots, complete the capture and
   image review, publish the screenshot assets and open the draft PR while the build, signing and brick test proceed. Do
   not wait for any of those outcomes before beginning this work. Give the subagent a copy of the approved section file
   in its own scratch subdirectory if the section has not landed on main yet (never the path of the file in your
   attended clone, which only you write), then send the changelog merge commit when it does. Its final website check
   must show the shots under the actual release entries, not merely pass a build that has no matching release section
   yet.

   Finalize the approved text too. This usually happens somewhere between step 4 and step 8, and the steps in between do
   not wait for it.
   - Make a finalization PR on the latest main, titled as `docs:` (it is not a `fix`, and needs no fragment), that
     replaces the `## vX.Y.Z` section of `CHANGELOG.md` with the section file, copied verbatim as step 2 describes, so
     the draft notice goes away. The heading must stay exactly as step 4 landed it, date included, because dist named
     the GitHub release after it; step 4 keeps the section file's heading in step with it. Add any guidance recorded
     earlier in this step and run `python3 releasing/check-changelog.py format`. When the tag's GitHub release already
     exists, also run `python3 releasing/finalize-release-notes.py --tag vX.Y.Z --ref <the PR's head commit> --dry-run`
     before merging, so a heading or notice slip shows up while it is still one PR to fix rather than two. Then merge it
     as part of this procedure. It goes to main only, never to the release branch, which keeps the draft the tag was
     built from.
   - Once that has merged and the tag's GitHub release exists (step 7), fetch and run
     `python3 releasing/finalize-release-notes.py --tag vX.Y.Z`. It replaces the draft notes in the release body with
     the section now on main, leaves the download tables and the release name alone, and refuses when the body is not
     what dist wrote. Run it again when in doubt; once finalized, it reports so and changes nothing. A GitHub release
     that is deleted and recreated (the recovery procedure in `dist-workspace.toml`) comes back with the tag's draft, so
     run it again after that.
   - If the approval changed which entries exist, which PRs they cite, or which entries are highlights (a promotion, a
     removal or a merge), bring the screenshot work from step 4 up to date the way `releasing/.agents/stable.md`,
     "Re-cutting a stable release", describes.

   If the maintainer approves before the changelog PR in step 4 merges, that PR carries the approved section without the
   notice instead, and there is nothing to finalize.
4. Make the changelog PR, including any announcement table additions or changes settled in the request reply per
   `releasing/AGENTS.md`, "In-app announcements". Attach new entries to `X.Y.Z`; keep entries from earlier candidates on
   their exact original versions and ids. Run that section's recorded table validation before merging a table edit. The
   PR also carries the drafted `## vX.Y.Z - <today>` section at the top of `CHANGELOG.md`, with the draft notice on the
   first line under the heading followed by the section file's categories, every fragment under `releasing/changelog.d/`
   deleted (including `kind: none` ones; they were for this sweep), `dprint fmt`, and
   `python3 releasing/check-changelog.py format` passing. If `<today>` differs from the date in the section file's
   heading, update the section file's heading to match what this PR lands, and regenerate the page with it, so that the
   finalization copy keeps the heading the release is named after. Merge it before going on.

   Send this merge commit to the screenshot subagent if it is already preparing the approved notes. Otherwise step 3
   starts it as soon as the notes are approved. Continue to step 5 without waiting for screenshot work. Its draft PR
   should be ready before the maintainer decides the release is good, rather than become another task after brick
   testing. It adds screenshots of visible changes, following `website/.agents/release-notes.md` and
   `website/EDITORIAL_RULES.md`. Give it this, in so many words:
   - Work in a new independent colocated jj checkout (its default workspace) based on the latest `main`, under a
     subdirectory of your scratch directory, never in the release checkout, which is busy with the release branch. Run
     `jjstack init` there; a linked workspace whose Git directory differs from jj's store fails its colocation guard. Do
     not run `cargo clean` or touch tags, the release branch, or anything on get.farhelm.io.
   - Read the approved section file supplied by the release agent, or the `## vX.Y.Z` section of `CHANGELOG.md` once it
     has landed, and decide which of its highlights (the entries above the folded "All changes" list, which gets no
     screenshots) change something a reader would see: a new dialog or control, or a redesign. Most entries do not, and
     a release with none gets no PR; say so and stop.
   - For each that does, add a shot to `scripts/docs-shots/release-notes.py` and an item to
     `website/src/release-notes/vX.Y.Z.json`. Run the full capture on Linux (`scripts/docs-screenshots.sh --local`, no
     `--only`), look at every image it prints and fix placement, then run `scripts/publish-docs-shots.sh` and confirm
     the website builds. After the approved changelog section lands, verify that the generated release page includes
     every new shot under its intended entry; a build can omit a listing whose release section has not landed yet.
   - Open the result as a draft PR per the `jjstack` skill, with its commit message cold-read per the
     `scode-commit-msg-reviewer` skill, and do not merge it. It starts no preview server, being unattended.

   The prompt also names the files the subagent reads in full first, from `releasing/AGENTS.md`, "Release screenshot
   subagent (fixed prompt)".

   The PR stays a draft until the maintainer confirms the release is good and going out. That confirmation authorizes
   the release agent to land it in step 8, without a separate merge ask; the screenshot subagent never merges it. A
   successful build or readiness to sign is not that confirmation. Pass its link on as soon as it is ready, with a note
   that the website changes only when someone deploys it. Screenshot preparation holds up no earlier release step.
5. Start a `release-X.Y.Z` branch at the changelog PR's merge commit EXACTLY, not at whatever main has become since:
   anything merged after it would ship in X.Y.Z without notes and leave its fragment behind as `STALE`. Confirm
   `releasing/changelog.d/` holds only `README.md` at that commit. Then bump the version to `X.Y.Z` in the root
   `Cargo.toml`'s `[workspace.package]` and `packaging/farhelm-desktop/dist.toml`, refresh `Cargo.lock`
   (`cargo metadata` suffices), and commit exactly those three files as `chore: release X.Y.Z`.
6. Before tagging: `cargo clean` (see `releasing/.agents/build-outputs.md`, "Build outputs"), then the version-parity
   tests (`cargo nextest run -p farhelm-helm --lib -E 'test(provisioning::assets)'` through the recorder),
   `dist plan --tag vX.Y.Z` naming BOTH packages under the version (a mismatch makes the desktop archive silently
   vanish), and `python3 releasing/check-changelog.py announce --tag vX.Y.Z`, which must print that dist's announcement
   matches.
7. Push the tag `vX.Y.Z` at the bump commit and watch the workflow to completion. Then verify the GitHub release: every
   asset the release contract names present and NO `SHA256SUMS` or `SHA256SUMS.minisig` (those belong on get.farhelm.io;
   the workflow refuses them), the release NOT marked prerelease, and the release page showing the changelog section
   above the download tables with the heading text as its name. Tell the maintainer it is ready to sign and publish at
   its version path, that its signed `SHA256SUMS` lists exactly the five files `releasing/AGENTS.md`, "Rules for every
   release", names (four archives and `install.sh`, no tmux builds), and hand them exactly this line, with the version
   filled in, to run from their release management location (outside this repository; agents never run it):

   ```
   ./bin/publish-release.sh vX.Y.Z && ./bin/deploy.sh
   ```

   It publishes the release at its version path without moving `/latest`. The maintainer asked for this one line on
   2026-10-10, for convenience. Once they have run it, check get.farhelm.io (read-only):
   `https://get.farhelm.io/vX.Y.Z/SHA256SUMS`, its `.minisig` and `install.sh` answer, and
   `https://get.farhelm.io/latest` still names the previous stable release.

   If the screenshot subagent from step 4 has reported by now, include its PR link (or its finding that the release
   needs no screenshots) when telling the maintainer the release is ready to sign. Also say whether the notes are
   finalized (step 3). If they are not, the maintainer decides whether to sign now or after; the agent does not hold the
   signing line back.
8. Stop for the brick test. The release is now staged: anyone can install it by naming its version, and nothing installs
   it by default. Tell the maintainer it is ready to brick test, and give them the line to hand the agent on the Mac
   test host: `brick test release vX.Y.Z` (`releasing/mac-vm-test/BRICK-TEST.md`). That test runs on the maintainer's
   Mac, in Tart VMs, so this session does not run it; it installs the staged release and upgrades from the newest
   previous stable releases at or above its upgrade floor (at most three), which is the check that supported updates do
   not brick existing installations (SPEC.md, "Upgrade compatibility and client scale"). Give the maintainer the risk
   report's ranked risks and manual checks as material for the test's per-release addendum. The result names P0's
   install, upgrade, supervisor and agent checks, P1's old-session and remote host menu checks, P2's live feedback send
   and bonus coverage separately; bring those results to the maintainer without deciding whether a P1 or P2 failure
   blocks promotion. Say whether the notes are finalized when you hand over the test. Wait for the maintainer's verdict.
   An unavailable test requires the maintainer's decision below.
   - Pass: finish step 3. If the release has screenshots and the verdict left it unclear whether the release is going
     out, notify the maintainer and ask for that confirmation. Once confirmed, finish any remaining screenshot work and
     land its PR using the normal validation and landing rules. That confirmation authorizes the merge. Land it before
     the deployment because the feed gets its screenshot links from that PR.

     Send the urgent notification specified in `releasing/AGENTS.md`, "Notifying the maintainer", and ask the maintainer
     to deploy the website from main. Wait for them to deploy it. The maintainer owns this deployment.

     Check the published data read-only, filling in `X.Y.Z` without the `v`. The feed must report format version 1 and
     list the exact release being promoted.

     ```sh
     notes=$(curl -fsS --max-time 20 -H 'Cache-Control: no-cache' https://farhelm.io/whats-new.json) &&
       jq -e --arg version 'X.Y.Z' '.formatVersion == 1 and any(.releases[]; .version == $version)' <<<"$notes"
     ```

     If the feed cannot be read or the release is absent, keep the handover pending and tell the maintainer what the
     check found. Once the check passes, hand the maintainer exactly this line with the version filled in. They run it
     from their release management location outside the repository.

     ```
     ./bin/set-latest.sh vX.Y.Z && ./bin/deploy.sh
     ```

     Once the maintainer says it has moved, check that `https://get.farhelm.io/latest` names `vX.Y.Z` and go on.
   - Fail: `/latest` stays on the previous release and the release is never promoted. Fix forward
     (`releasing/.agents/fixing-forward.md`, "Fixing forward") and cut the next patch version. Whether to take the
     staged version off get.farhelm.io is the maintainer's call.
   - Could not run: ask the maintainer whether to fix the test environment and run it again, or to promote without it.
     If they approve promotion without the test, follow the full Pass sequence, including finalization, screenshot
     landing and the deployed feed check.
9. Hand the maintainer the ordinary install command and remind them to quit the desktop app before updating.
10. `cargo clean` again (see `releasing/.agents/build-outputs.md`, "Build outputs").

Step 8 needs finalized notes because the installed app waits for that release to appear in the feed. If wording review
is still open, the release can stay staged until the maintainer approves the notes and deploys the website.

A failure anywhere in this procedure, from the pre-tag checks through the brick test, is fixed forward without asking
(`releasing/.agents/fixing-forward.md`, "Fixing forward"). A failed tag build publishes nothing, and a release that
failed its brick test was signed and staged but never promoted; either way its tag name is spent and the next attempt is
the next patch version. The abandoned release branch is left as it is, like the earlier ones.

## Re-cutting a stable release

A stable re-cut cannot reuse the old release branch: its notes have to cover what now rides along. Re-run step 1's sweep
and protocol check against the new base, then make another changelog PR as in step 4. Carry the settled announcement
table over without asking again, retaining each entry's version and id even when the re-cut gets a new patch version; do
not copy an earlier announcement into a new entry. The PR renames the `## vX.Y.Z` section when the version changed (a
stable `announce` fails unless the newest section names the tag exactly), considers the fix and all required commits
that landed since, and deletes the fragments. Entries the maintainer already approved stay word for word, in the tier
they were approved in. Draft new full-list entries from the fragments (or, for a `MISSING` commit, from the commit and
its PR) following `releasing/EDITORIAL_GUIDANCE.md`, marking any proposed highlights, with a newly landed plan's release
note used as a highlight and checked as step 2 does, and sort them into categories as step 2 does. Leave out
`kind: none` fragments and `STALE` ones rather than stopping to ask about them. Re-run the `plans` mode too, and
reconcile what it lists with the carried-over section by PR number: a plan whose PRs the section already cites in the
highlights keeps its entry as it stands, and only a plan with PRs the section does not cite yet gets a new entry. A plan
whose PRs the section cites only in the full list (its PRs merged before its landing was recorded, so the earlier
attempt curated them from their fragments) now has its note: its entry becomes a highlight and the full-list entries
stay, unless the maintainer already approved the section without that work highlighted, in which case put the plan's
entry on the page with a callout instead of adding it. When the failed attempt pushed its tag, the mode's range starts
at that attempt's changelog commit, so "merged before this range" there means the failed attempt covered it, not that it
shipped. The maintainer chose this on 2026-10-11, over stopping for wording review and over cutting from the old base
with only the fix cherry-picked.

The re-cut's section is therefore a draft again, whether or not the previous attempt's notes were finalized: it opens
with the draft notice in `CHANGELOG.md`, and its review and finalization follow steps 2 and 3. If the new version
requires headlines and a carried-over highlight still needs one, propose a headline for approval on the review page. The
section file and the review page still hold the section without the notice, as step 2 says: when the re-cut takes a new
version, rename the section file to it in the attended clone (`git mv`), then update both with the re-cut's entries
rather than starting new ones (serve the page again as step 2 does if its server was stopped), and in the message that
hands the maintainer the updated page, list every entry added without their review, every proposed highlight, every
fragment left out, and every callout on a plan's entry.

Then continue from step 5 at the new changelog PR's merge commit. A `release-X.Y.Z` branch from an attempt that never
pushed a tag is replaced, since nothing points at it; one whose tag was pushed is left as it is. A GitHub release from
an earlier attempt keeps whatever notes it had; only the attempt being promoted gets finalized.

Carry the screenshot work forward as soon as the re-cut is settled. Continue the same subagent, or start one with step
4's instructions that updates the existing draft PR rather than opening a second. A failed attempt does not put this
work on hold: assume the next attempt ships, rename `website/src/release-notes/vX.Y.Z.json` and the shots' version, and
assess any new visible entries as they become available. Keep unchanged approved entries; do not wait for another brick
test, the new changelog merge or repeated approval of unchanged text to resume preparation. Send the exact changelog
merge commit when it lands, so the subagent can finish its generated-page check against the current notes. Continue from
step 5 at that commit without waiting for screenshot work. Its PR still stays draft until the maintainer confirms the
release is good and going out.
