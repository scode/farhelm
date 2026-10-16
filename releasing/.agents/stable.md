# Cutting a stable release

The stable release procedure, step by step, and how a stable release is re-cut after a failed attempt. It is the main
file of the stable release reading list in `releasing/AGENTS.md`, and the fixing forward and stable re-cuts and
changelog curation reading lists include it. Step numbers in this file refer to the numbered steps of
`releasing/.agents/stable.md`, "Cutting a stable release".

> NOTE: In an unattended run (plan executor, landing monitor, delegate), an instruction here to ask, confirm with, or
> propose to the maintainer follows `AGENTS.md`, "Unattended runs".

A stable release is cut from main. The changelog section lands on main FIRST, as its own PR; the release branch is cut
after it merges, so the bump commit keeps its three-file shape and main is the only place the notes ever live. The
section lands as a draft, and the procedure does not wait for the maintainer to approve its wording: the tag builds, and
the release can be signed and brick tested, while they review it. The approved text then replaces the draft on main and
on the GitHub release (step 3). The maintainer asked for this on 2026-10-13. Nothing that ships reads the notes (the
app's What's new item opens the docs website, which builds from main), so what the wait bought was that the GitHub
release and the tag's own copy of `CHANGELOG.md` never held a draft. The draft notice covers the release page (and the
notification GitHub sends repository watchers) until finalization; the tag's copy keeps the draft for good, which
nothing reads. Main holds the draft too until finalization, so a docs website deployed in that window shows it on the
Release notes page, which is where the app's What's new item lands. When asked to deploy the website while the notes are
still a draft, say so before deploying.

1. Fetch tags, then run `python3 releasing/check-changelog.py fragments`. Go through its report with the user: every
   required commit since the last stable release is included unless the user explicitly excludes it, `MISSING` ones get
   an entry written now from the commit and PR, `kind: none` fragments are shown to the user as proposed exclusions (the
   fragment author's judgment is not the user's decision), and `STALE` ones are raised as described in
   `releasing/.agents/changelog.md`, "The checker". This is the sweep the fragment rule exists to make cheap.

   In the same step, ALWAYS check whether the helm–supervisor protocol version changed since the last stable release,
   and tell the user the answer either way. Compare `PROTOCOL_VERSION` in `crates/farhelm-proto/src/lib.rs` at the merge
   base the sweep uses and at the release base, and list the commits that changed it:

   ```
   base=$(git merge-base vPREV origin/main)
   git log --oneline -G 'PROTOCOL_VERSION: u32 = [0-9]+' "$base"..origin/main -- crates/farhelm-proto/src/lib.rs
   ```

   (`vPREV` is the last stable tag.) Neither the sweep nor the fragments can be trusted to surface this: a bump often
   rides in a `refactor`, which needs no fragment, or inside a feature whose fragment is about something else. When the
   version changed, the section carries the fixed remote-hosts entry described in `releasing/.agents/changelog.md`,
   "Format", citing those commits' PRs; when it did not, it carries no such entry.

   Start the risk report (`releasing/.agents/risk-report.md`, "The release risk report") now too. It must reach the
   maintainer before the tag is pushed in step 7.
2. Read `releasing/EDITORIAL_GUIDANCE.md`, then draft the section from the fragments and from whatever the user and the
   agent agree on in conversation, following that guidance. Sort every fix into Fixes worth highlighting or Misc fixes
   by the criteria in `releasing/.agents/changelog.md`, "Format"; the split is the agent's first call and the
   maintainer's to change. Keep the whole proposed `## vX.Y.Z - YYYY-MM-DD` section in a Markdown file in your scratch
   directory, written exactly as it will go into `CHANGELOG.md`, and publish it to the maintainer as a snippet with the
   `scode-snippets` skill, giving them its link. If you do not know what the `scode-snippets` skill is, stop and tell
   the maintainer; do not substitute another way of showing the draft. The maintainer asked for snippets on 2026-10-13,
   replacing a local file they edited by hand. They review the snippet and give their changes in conversation. Apply
   them to the scratch file and update the same snippet after each round, until the maintainer approves the text. The
   scratch file and the snippet hold the section as it will read once approved, without the draft notice; `CHANGELOG.md`
   gets the notice added in step 4.

   Do not wait for approval: go on to step 4 as soon as the first snippet is published, and keep the review going
   alongside the rest of the procedure. Step 3 says how the approved text lands.

   Treat any wording the maintainer supplies as authoritative. Reproduce it exactly: do not summarize, paraphrase,
   polish, reorder, or silently correct it. When the maintainer gives edits to the agent's proposed wording, apply
   precisely those edits, without interpreting them as permission for other changes. An explicit request to write or
   rewrite text grants that latitude only for the requested text. Ask when an instruction is ambiguous. Format or
   checker problems must be brought back to the maintainer rather than silently changing reviewed wording. When
   transferring the approved section into `CHANGELOG.md`, copy the scratch file verbatim, including headings, blank
   lines, and every word; add only the separation needed between the section and the surrounding changelog sections.
   Verify the copy by comparing it against the scratch file. Do not run a formatter over the approved section.
3. While iterating, watch for feedback that generalizes beyond the entry it was given on: a word the maintainer calls
   internal jargon, a shape of sentence they keep rewriting, a kind of detail they keep cutting or adding. Two ways a
   rule gets into `releasing/EDITORIAL_GUIDANCE.md`, and only two: the maintainer states it as a rule in so many words
   ("X is internal jargon, use Y"), in which case record it; or the agent infers a generalization from the edits, in
   which case ASK, naming the rule it would write, and record it only on a yes. Never write an inferred rule on the
   agent's own judgment; the file is the maintainer's opinions, and a guessed one would steer every future draft. Record
   each rule in the PR that lands the approved text (this step's finalization PR, or the changelog PR in step 4 when
   approval came first), with the example that prompted it. Feedback that only fixes the one entry is applied and not
   recorded.

   When the maintainer approves the text, finalize it. This usually happens somewhere between step 4 and step 8, and the
   steps in between do not wait for it.
   - Make a finalization PR on the latest main, titled as `docs:` (it is not a `fix`, and needs no fragment), that
     replaces the `## vX.Y.Z` section of `CHANGELOG.md` with the scratch file, copied verbatim as step 2 describes, so
     the draft notice goes away. The heading must stay exactly as step 4 landed it, date included, because dist named
     the GitHub release after it; step 4 keeps the scratch file's heading in step with it. Add any guidance recorded
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
   - If the approval changed which entries exist or which PRs they cite, bring the screenshot work from step 4 up to
     date the way `releasing/.agents/stable.md`, "Re-cutting a stable release", describes.

   If the maintainer approves before the changelog PR in step 4 merges, that PR carries the approved section without the
   notice instead, and there is nothing to finalize.
4. Make the changelog PR: the drafted `## vX.Y.Z - <today>` section at the top of `CHANGELOG.md`, with the draft notice
   on the first line under the heading followed by the scratch file's categories, every fragment under
   `releasing/changelog.d/` deleted (including `kind: none` ones; they were for this sweep), `dprint fmt`, and
   `python3 releasing/check-changelog.py format` passing. If `<today>` differs from the date in the scratch file's
   heading, update the scratch file's heading to match what this PR lands, and the snippet with it, so that the
   finalization copy keeps the heading the release is named after. Merge it before going on.

   The moment it merges, start a subagent to give the release's notes their screenshots on the docs website, and go on
   to step 5 without waiting for it. The maintainer asked for this on 2026-10-13: the screenshot work used to start only
   once the release was ready to sign, which left a separate wait for it after everything else was done. Started here,
   it runs while the tag builds and the release is signed and brick tested, so its draft PR is ready by the time the
   release is promoted. The notes' text is already on the website, built from the changelog section this step merged
   (still the draft, which step 3 replaces); what the subagent adds is screenshots of the release's visible changes,
   following `website/.agents/release-notes.md` and `website/EDITORIAL_RULES.md`. Give it this, in so many words:
   - Work in a new jj workspace based on the latest `main`, under a subdirectory of your scratch directory, never in the
     release checkout, which is busy with the release branch. Do not run `cargo clean` or touch tags, the release
     branch, or anything on get.farhelm.io.
   - Read the `## vX.Y.Z` section of `CHANGELOG.md` and decide which entries change something a reader would see: a new
     dialog or control, or a redesign. Most entries do not, and a release with none gets no PR; say so and stop.
   - For each that does, add a shot to `e2e/docs-shots/release-notes.spec.ts` and an item to
     `website/src/release-notes/vX.Y.Z.json`, run the full capture (`scripts/docs-screenshots.sh`, no `--only`), look at
     every image it prints and fix placement, then run `scripts/publish-docs-shots.sh` and confirm the website builds.
   - Open the result as a draft PR per the `jjstack` skill, with its commit message cold-read per the
     `scode-commit-msg-reviewer` skill, and do not merge it. It starts no preview server, being unattended.

   The prompt also names the files the subagent reads in full first, from `releasing/AGENTS.md`, "Release screenshot
   subagent (fixed prompt)".

   The PR stays a draft until the maintainer asks for it to land; nothing in this procedure merges it. Pass its link on
   to the maintainer when the subagent reports it, with a note that the website changes only when someone deploys it.
   The subagent's result holds up no step of the release.
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
   its version path, and hand them exactly this line, with the version filled in, to run from their release management
   location (outside this repository; agents never run it):

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
   Mac, in Tart VMs, so this session does not run it; it installs the staged release and upgrades the previous stable
   releases to it, which is the check that the release does not brick existing installations (SPEC.md, "Upgrade
   compatibility and client scale"). Give the maintainer the risk report's ranked risks and manual checks as material
   for the test's per-release addendum. The result names P0's install/upgrade/agent checks, P1's live feedback send and
   bonus coverage separately; bring those results to the maintainer without deciding whether a P1 failure blocks
   promotion. If the notes are not finalized yet, say so in the same message, since a pass leads straight to promotion.
   Then wait for the maintainer's verdict; do not treat silence, or a test that could not run, as a pass.
   - Pass: the maintainer moves `/latest`, which no agent does. If the notes are still not finalized when they report
     the pass, remind them before they move it, since promoting would send every ordinary install to a release whose
     page still reads DRAFT. Once they say it has moved, check that `https://get.farhelm.io/latest` names `vX.Y.Z`, and
     go on.
   - Fail: `/latest` stays on the previous release and the release is never promoted. Fix forward
     (`releasing/.agents/fixing-forward.md`, "Fixing forward") and cut the next patch version. Whether to take the
     staged version off get.farhelm.io is the maintainer's call.
   - Could not run: ask the maintainer whether to fix the test environment and run it again, or to promote without it.
     Only the maintainer decides to skip the test.
9. Hand the maintainer the ordinary install command and remind them to quit the desktop app before updating.
10. `cargo clean` again (see `releasing/.agents/build-outputs.md`, "Build outputs").

The procedure is not finished until step 3's finalization has run, or the maintainer has said to leave it for later.
When everything else is done and the notes are still a draft, say so plainly in the final report rather than reporting
the release as complete.

A failure anywhere in this procedure, from the pre-tag checks through the brick test, is fixed forward without asking
(`releasing/.agents/fixing-forward.md`, "Fixing forward"). A failed tag build publishes nothing, and a release that
failed its brick test was signed and staged but never promoted; either way its tag name is spent and the next attempt is
the next patch version. The abandoned release branch is left as it is, like the earlier ones.

## Re-cutting a stable release

A stable re-cut cannot reuse the old release branch: its notes have to cover what now rides along. Re-run step 1's sweep
and protocol check against the new base, then make another changelog PR as in step 4. It renames the `## vX.Y.Z` section
when the version changed (a stable `announce` fails unless the newest section names the tag exactly), adds entries for
the fix and for every required commit that landed since, and deletes the fragments. Entries the maintainer already
approved stay word for word. The new ones are not held for the maintainer's approval: draft them from the fragments (or,
for a `MISSING` commit, from the commit and its PR) following `releasing/EDITORIAL_GUIDANCE.md`, sort them into
categories as step 2 does, and add the fixed remote-hosts entry if the protocol version changed in the new commits.
Leave out `kind: none` fragments and `STALE` ones rather than stopping to ask about them. The maintainer chose this on
2026-10-11, over stopping for wording review and over cutting from the old base with only the fix cherry-picked.

The re-cut's section is therefore a draft again, whether or not the previous attempt's notes were finalized: it opens
with the draft notice in `CHANGELOG.md`, and its review and finalization follow steps 2 and 3. The scratch file and the
snippet still hold the section without the notice, as step 2 says: update both with the re-cut's entries rather than
starting new ones, and in the message that hands the maintainer the updated snippet, list every entry added without
their review and every fragment left out.

Then continue from step 5 at the new changelog PR's merge commit. A `release-X.Y.Z` branch from an attempt that never
pushed a tag is replaced, since nothing points at it; one whose tag was pushed is left as it is. A GitHub release from
an earlier attempt keeps whatever notes it had; only the attempt being promoted gets finalized.

The screenshot work from step 4 follows the notes it was based on. When the re-cut's changelog PR merges, bring it up to
date in the same way step 4 starts it: continue the screenshot subagent if it is still working, or start a new one with
step 4's instructions that updates the existing draft PR (or opens one, if there was none) rather than opening a second.
Tell it what changed: a new version means renaming `website/src/release-notes/vX.Y.Z.json` and the shots' version, and
new entries need the same decision about whether a reader would see them. Then continue from step 5 at the new changelog
PR's merge commit without waiting for it.
