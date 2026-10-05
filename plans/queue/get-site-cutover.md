# get.farhelm.io cutover: installs and updates from one signed origin

Written against main at 457897da on 2026-10-05. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file directs a change to them.

## The goal

Farhelm's installer, the helm's payload downloads, and the Mac app's updater all switch to one origin,
`https://get.farhelm.io`, and the release signature becomes the thing installed software trusts. Today:

- `scripts/install.sh` asks GitHub which release is latest (`releases/latest`), and trusts the `SHA256SUMS` sitting on
  the GitHub release beside the archives. People run it from main's raw file.
- The helm downloads its own version's payloads from the GitHub release and verifies `SHA256SUMS.minisig` with one
  compiled-in key (`MINISIGN_PUBKEY` in `crates/farhelm-helm/src/provisioning/release_payloads.rs`).
- CI signs `SHA256SUMS` in `.github/workflows/sign-sums.yml` with a key held as a GitHub repository secret.
- The Mac app's updater (`crates/farhelm-ui/src/desktop/updater.rs`, landed in #1600 and not yet in any release) asks
  GitHub's `releases/latest`, downloads `scripts/install.sh` from main, and runs it with no signature check at all.

Agents working on this project can push to main and run workflows, so every one of those is a path from an agent's push
to code running on users' machines. The maintainer has set up `https://get.farhelm.io` as a static site whose content
lives outside GitHub on a trusted host, and moved signing there. This plan makes the Farhelm side match, so that the
next release is the last one that breaks installer and updater continuity: after it, everything else (accounts, DNS, the
signing ceremony) can be hardened on the server side without changing any client.

This plan covers four of the five pieces of that cutover. The fifth (switching the README, the website and
`docs/install_uninstall.md` to the new install command) waits until the first release is published on the site and is
NOT part of this plan. Do not touch the README, `website/`, or `docs/install_uninstall.md`.

### The site's layout (fixed; the site already serves it)

These paths are a permanent contract with every client. Use exactly these:

- `https://get.farhelm.io/install.sh`: the installer of the latest stable release.
- `https://get.farhelm.io/latest`: one line, the latest stable tag with its `v` (`v0.22.0`), followed by a newline.
  Never a prerelease. Returns 404 before the first release is published there.
- `https://get.farhelm.io/<tag>/SHA256SUMS`: checksums of that release's six payloads (the `SUMS_MEMBERS` in
  `sign-sums.yml`) plus its `install.sh`, in `sha256sum` format, sorted by name.
- `https://get.farhelm.io/<tag>/SHA256SUMS.minisig`: the minisign signature over that file, trusted comment exactly
  `farhelm <tag>` (unchanged from today's rule: `farhelm v{version}`).
- `https://get.farhelm.io/<tag>/install.sh`: the installer as signed with that release.
- `https://get.farhelm.io/<tag>/<payload>` for each of the six payload names: a temporary (307) redirect to the GitHub
  release asset. Clients follow it; integrity never depends on where it points.

`<tag>` is `v` plus the version, including prerelease suffixes (`v0.22.0-rc.7`). Prereleases are published under their
tag but never named by `latest`.

### Acceptance criteria

The release key ring (PR 1):

- Installed software trusts a ring of public keys instead of one: a signature is accepted when it verifies under any key
  in the ring AND its trusted comment is exactly `farhelm v{version}`. Nothing else about verification loosens.
- The production ring holds only today's key for now (see M2). The committed `.pub` file(s) and the compiled-in ring
  stay in exact agreement, checked by a test in both directions, so adding the maintainer's two keys later is a change
  to the key files and the constant and nothing else.
- Each key in the ring is held as its verbatim base64 public-key line (line 2 of a minisign `.pub` file) in a string
  constant reachable from both shipped binaries, `farhelm` (the helm's provisioning path) and `farhelm-desktop` (the
  updater). The maintainer's publishing tool refuses to sign a release unless it finds every key string verbatim in
  every archive, so a ring stored any other way (decoded bytes, split strings) would make every release unsignable.
- Tests cover a multi-key ring: a signature by the second key verifies, a signature by a key outside the ring is
  refused, and the trusted-comment rule still holds for every key.
- One public verification entry point in `farhelm-helm` (planner proposal: a function taking the sums bytes, the
  signature bytes and the expected version, returning the parsed name-to-digest map) that the helm and the updater both
  use, with the ring injectable for tests the way `verify_sums` already takes its key.

The origin switch (PR 2):

- The helm's default release base URL is `https://get.farhelm.io/v{version}/` (`default_release_base_url` in
  `crates/farhelm-helm/src/provisioning/payloads.rs`), so `SHA256SUMS` and the signature come from the site and payload
  downloads follow its redirect. `--release-base-url` / `FARHELM_RELEASE_BASE_URL` keep working as overrides. Messages
  that point users at the GitHub releases page say something accurate for the new origin.
- `scripts/install.sh`:
  - With `FARHELM_VERSION` set, installs that version as today (same version grammar, same refusals).
  - Without it, reads `https://get.farhelm.io/latest`, validates it with the existing version grammar, and refuses a
    prerelease or anything malformed. It no longer asks GitHub anything.
  - Reads `SHA256SUMS` from `https://get.farhelm.io/<tag>/SHA256SUMS`, unless `FARHELM_INSTALL_SUMS_FILE` names a local
    file, in which case it reads that file instead and fetches no checksums at all (the updater uses this; M4).
  - Downloads archives from `https://get.farhelm.io/<tag>/<archive>`, following the redirect, HTTPS only for every hop
    (the existing `--proto`/`--proto-redir` discipline).
  - A tag whose `SHA256SUMS` is not on the site (404) fails before anything is downloaded or changed, with one clear
    line saying that release is not published on get.farhelm.io (M5).
  - `FARHELM_INSTALL_TEST_BASE_URL` keeps working for the test harness, now standing in for the whole get.farhelm.io
    origin (`latest`, checksums and archives).
  - Everything else the installer does today (asset table, archive validation, app bundle assembly, locking, messages,
    Linux refusal) is unchanged.
- `scripts/test-install-sh.sh` serves the new layout from its fixture server and covers: latest from the site, a pinned
  version, a prerelease named by `latest` refused, a malformed `latest` refused, the 404 message, and
  `FARHELM_INSTALL_SUMS_FILE` used instead of any checksum fetch (the fixture must prove no `SHA256SUMS` request was
  made). Existing coverage keeps passing.
- `scripts/test-uninstall.py` serves its fixture releases in the new layout too. Today it sets
  `FARHELM_INSTALL_TEST_BASE_URL` to a directory and serves `SHA256SUMS` and the archives directly under it; with the
  variable now standing in for the whole origin, the installer asks for `<origin>/v<version>/SHA256SUMS`, including the
  `v0.0.0-unreleased` case the installer accepts only with the test base URL. This suite runs only on macOS, in the
  release gate and in `ci.yml`'s `uninstall-macos` suite, so a Linux executor will not see it fail: validate it with
  `gh workflow run ci.yml --ref <bookmark> -f suite=uninstall-macos` (an on-demand CI run is allowed; it is not the
  release workflow), or say plainly in the report that it is unvalidated.
- The spec passages PR 2 makes untrue change in PR 2: SPEC.md "Topology" (the release source is GitHub by default), the
  installer-trust paragraph of "Installation and updates" (it trusts GitHub over TLS), and the release-download sentence
  of "Security"; SPEC_impl.md's CLI section where it describes the default download source, and its provisioning text
  where the helm downloads "from the GitHub release matching its own version". Search both files for every mention of
  GitHub as a download source rather than relying on this list.

The release pipeline (PR 3):

- CI no longer signs anything and never sees a signing key. `sign-sums.yml` keeps its `validate` job (inventory, archive
  structure, static linkage, the release-shaped binary check) and loses the `sign` job and everything only it needed
  (minisign download, the secret, the upload). The inventory no longer expects `SHA256SUMS` or `SHA256SUMS.minisig` on
  the GitHub release; they are published only on get.farhelm.io now, so their presence on a GitHub release is
  unexpected. Whether the workflow file keeps its name is your call (renaming means regenerating `release.yml`; prefer
  keeping the name and fixing its header comment). `dist generate --check` passes.
- `releasing/AGENTS.md`, the `dist-workspace.toml` header, and `assets.rs`'s comments describe the new flow: CI builds
  and publishes archives on GitHub; the maintainer then signs and publishes on the trusted host with tooling that lives
  outside this repository on purpose; an RC is installed with
  `curl -fsSL https://get.farhelm.io/<tag>/install.sh | FARHELM_VERSION=<tag> sh`. Every step that checks for
  `SHA256SUMS` on the GitHub release now checks get.farhelm.io instead.
- SPEC_impl.md "Verification chain (D3)" and "Release signing key" describe the ring, the new origin, `install.sh` in
  the signed set, signing on the maintainer's trusted host, and rotation. State the rotation rules exactly, since they
  are what keep a rotation from stranding installed apps:
  - Releases are signed with the primary key; the backup is compiled in but kept offline.
  - A release's signing key must be in that release's own ring (the helm verifies its own release with its own ring, so
    a release signed by a key it does not carry cannot provision hosts) and in the ring of the release before it (so
    apps can update to it).
  - A rotation therefore replaces at most one key per release: to retire the primary A of ring {A, B}, the next release
    carries {B, C} and is signed with B, and B becomes the primary. A compromised key is retired the same way, promptly.
  - An app more than one rotation behind cannot verify the latest release and must be reinstalled with the curl command,
    which trusts get.farhelm.io over TLS and needs no key. That is also the recovery if both keys are lost. Space
    consecutive rotations so that daily automatic updates have time to carry apps across each one.

  Remove the text about the secret living in GitHub and about the updater not verifying signatures.

The updater (PR 4):

- It learns the latest version from `https://get.farhelm.io/latest` (a plain GET, small size cap, existing timeout
  discipline), refuses a prerelease or malformed answer, and keeps today's newer-than-installed rule.
- To install version X it downloads `https://get.farhelm.io/vX/SHA256SUMS` and `.minisig`, verifies them through the
  shared entry point from PR 1 (ring plus trusted comment `farhelm vX`), downloads
  `https://get.farhelm.io/vX/install.sh`, and refuses unless its SHA-256 equals the signed `install.sh` entry (a missing
  entry is a refusal). Only then does it run that script, pinned with `FARHELM_VERSION`, with
  `FARHELM_INSTALL_SUMS_FILE` pointing at the verified checksum bytes written to the same private temporary directory.
  Nothing it runs or installs is trusted on TLS alone.
- The existing scrub of `FARHELM_*` from the installer's environment stays, so no ambient variable (in particular
  `FARHELM_INSTALL_TEST_BASE_URL`) reaches the script; it sets only the two variables above.
- When a release fails verification (no key in the ring verifies its signature, its trusted comment is wrong, or its
  `install.sh` does not match the signed checksum), the app cannot update itself, and the user must find out (M10). The
  version readout shows a lasting notice, whether the check was automatic or the user's, saying in plain words that this
  Farhelm can no longer verify its updates and should be reinstalled with
  `curl -fsSL https://get.farhelm.io/install.sh | sh`. It stays until a later check verifies and installs, or finds
  nothing newer that fails. Network and HTTP failures are not this: they keep today's behavior (logged for automatic
  checks, shown for the user's own).
- Everything else about the updater is unchanged: activation, single flight, schedule, published state, the Installed
  record as the judge of success, detaching the installer from the app.
- Unit tests cover: a valid release installs; a bad signature, a wrong trusted comment, an `install.sh` whose hash does
  not match, a missing `install.sh` entry, a prerelease or malformed `latest` are each refused with nothing run. Test
  this at the install step's own seam (planner proposal: the install function takes a fetch function and a run function
  and verifies before it calls run) rather than adding slots to the worker's `Deps`: a verification failure already
  surfaces as a failed install that the Installed record judges, so the worker itself does not change.
- SPEC.md "Installation and updates" and "Security" (the passage saying the updater asks GitHub and does not verify the
  release signature), and SPEC_impl.md "The desktop app's updater", describe the new behavior.
- SPEC_impl.md's updater section records, beside the site layout, that the installer's interface to the updater is a
  permanent contract: every shipped updater runs future releases' `install.sh` with exactly `FARHELM_VERSION` and
  `FARHELM_INSTALL_SUMS_FILE`, and with a checksum file that may list more entries than the installer needs. A later
  installer that renames or drops either variable, or rejects extra entries, breaks every installed app's updates.

The last PR of the stack also narrows the TODO.md entry "Lock down the release and update trust chain" (M8).

## Decisions already made

The maintainer's own words are quoted where they settle something.

- **M1. One origin, fixed layout.** Clients only ever talk to get.farhelm.io, using exactly the layout above. Payloads
  are fetched through the site's redirect rather than from a hard-coded GitHub URL so the hosting can move later without
  touching clients. "I want to get to the point of 'the last time we break the installer continuity' by switching to
  this new system, and then harden separately on the server side."
- **M2. A two-key ring, with a placeholder for now.** Clients compile in a primary and a backup key and accept either;
  this is what lets a lost or compromised primary be retired without a transition release that every client has to
  catch. The maintainer generates both keys on the trusted host and swaps them in afterwards, outside this plan. Until
  then the production ring holds only today's key, so nothing in this plan changes which signatures current releases
  need. The maintainer's publishing tooling refuses to sign a release whose binaries do not carry both real keys, so a
  release built with the placeholder ring can never be signed by mistake. Do not invent, generate, or commit any new
  production key. Throwaway test keys are fine and expected: the release fixtures' secret key was never committed
  (`crates/farhelm-helm/tests/fixtures/release/README.md`), so new signed fixtures (an `SHA256SUMS` with an `install.sh`
  line, for PR 4) need a fresh throwaway pair generated per that README. PR 1's multi-key tests need no new key: a ring
  of today's key plus the fixtures' test key verifies the existing fixtures, and a ring of today's key alone refuses
  them.
- **M3. CI never signs.** Signing happens on the maintainer's trusted host, with tooling that deliberately lives outside
  this repository so that agents cannot author it. Do not add signing, publishing or deploy scripts for get.farhelm.io
  to this repository. Do not delete the `MINISIGN_SECRET_KEY` GitHub secret, push tags, run the release workflow, deploy
  anything, or contact get.farhelm.io beyond read-only requests; those are the maintainer's.
- **M4. The updater runs only a signed installer, fed signed checksums.** `install.sh` joins the signed set so the
  updater can verify the script before running it, and the verified checksums are handed to the script so every archive
  it downloads is checked against signed hashes, not against a second fetch over TLS. Installing logic compiled into the
  app was considered and not chosen: the signed script keeps one installer for both paths.
- **M5. No backfill.** Releases published before the switch are not republished on get.farhelm.io (their checksums were
  signed with a key that has been readable by CI). The installer refuses a version the site does not have, with a clear
  message, rather than falling back to GitHub. No version floor is hard-coded; the 404 is the signal.
- **M6. Breaking the old paths is fine.** "Don't worry about breaking existing 'users' (== just me for now)." Once PR 2
  lands, main's `scripts/install.sh` reads from get.farhelm.io, which serves no release until the maintainer publishes
  one; installs from main fail until then. That is accepted. No compatibility shims for the old GitHub-based flow.
- **M7. PR 5 is out.** README, `website/`, and `docs/install_uninstall.md` keep the old command; they switch after the
  first release is on the site.
- **M8. Narrow the TODO entry.** The last PR rewrites "Lock down the release and update trust chain" to what remains:
  swapping in the maintainer's two real keys; switching the README, website and install docs to
  `curl -fsSL https://get.farhelm.io/install.sh | sh`; deleting the old `MINISIGN_SECRET_KEY` GitHub secret; a Vercel
  user that holds only the get.farhelm.io project; disconnecting the docs project from Git (its automatic deploys are
  off only by a setting in agent-writable `website/vercel.json`); tag rulesets and GitHub immutable releases; locking
  down the DNS, registrar and email accounts; and reviewing the source diff before signing, since the signature proves
  who published a release, not that its code is sound. Keep it in the maintainer's terse TODO style and keep its bucket,
  and drop its ``Plan: `plans/queue/get-site-cutover.md`.`` reference, since what remains is not planned.
- **M9. Review gate:** two fresh-context reviewers per PR, Opus 5.5 at high effort and gpt-6-astra at high effort, on
  the general charter below. No review swarm.
- **M10. Rotations may strand very stale apps; they must say so.** With two keys, an app that misses two key rotations
  cannot verify the latest release. The maintainer accepted that limit ("nah thats fine") rather than multi-signature
  releases or a bigger ring; the recovery is rerunning the curl installer, which needs no key. But such an app must not
  fail silently, which today's updater would do for an automatic check: "the 'needs reinstall' notice is probably good
  to have regardless". Hence the lasting readout notice in PR 4.

## Implementation outline

Four PRs, in this order. Each builds on the previous one.

1. **Key ring.** `release_payloads.rs`: the single `MINISIGN_PUBKEY` becomes a ring (planner proposal: `&[&str]`) and
   `verify_sums` tries each key, keeping its refusal messages; the cached-control check (`cached_sums_verify`) uses the
   same path. Add the public entry point. Adjust the key-file parity test (the `.pub` file beside the module) to cover
   every key in both directions. SPEC_impl text for the ring can wait for PR 3, which rewrites those sections whole.
   Probably `feat` with a `kind: none` fragment, or `refactor`; follow root `AGENTS.md`.
2. **Origin switch.** `payloads.rs` default base URL and its tests and messages; `scripts/install.sh` version discovery,
   checksum source, archive URLs and messages; `scripts/test-install-sh.sh`; the provisioning tests that spell the
   GitHub URL. `assets.rs` parity tests should keep passing unchanged (the asset table does not change); if any of them
   asserts the six-member `SHA256SUMS`, it now must allow the `install.sh` entry. A user-visible change, with a
   changelog fragment.
3. **Pipeline.** `sign-sums.yml`, `dist-workspace.toml` header, `releasing/AGENTS.md`, SPEC_impl "Verification chain
   (D3)" and "Release signing key". `ci` or `docs` typed as root `AGENTS.md` dictates.
4. **Updater.** `updater.rs` probe and install path, its tests, SPEC.md and SPEC_impl updater sections, and the TODO.md
   narrowing (M8). `feat`, with a changelog fragment.

The existing facilities to reuse: `verify_sums`, `parse_sums` and the trusted-comment rule in `release_payloads.rs`; the
updater's `Deps` injection, `run_installer`, `private_temp_dir` and `farhelm_variables` scrub; the installer's
`curl_get` wrapper, version grammar and fixture server. Nothing here needs a new subsystem: no metadata format beyond
the files above, no new persistent state, no key management code.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is `farhelm-plan-get-site-cutover-log.md`
in the parent directory of the checkout you run in, derived as that section says. Resolve it to an absolute path before
you start. Scratch files go in the agent scratch directory root `AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
TODO.md and `plans/queue/INDEX.md`) rather than starting over. If it does not exist, this is a fresh start. A plan that
an earlier executor worked on, or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing
one plan, step 7) before any work.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain. The sub-agents `plans/AGENTS.md`
requires of every plan (the resume check, and the cold reads of a blocked question and of the report) are exempt from
the no-delegation demand and run as that file says, not through galaxy-brain.

### Resource watchdog

Immediately after activating galaxy-brain, start a resource watchdog as a background process (not an agent) and keep it
running for the whole run. Every 60 seconds it samples free space on the filesystems holding the checkout, the agent
scratch directory and `/tmp`, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat`
and `sysctl` on macOS). It writes each sample to a private heartbeat/status file in the scratch directory, and it emits
a notification (in Claude Code, a stdout line of a monitor started with the Monitor tool; elsewhere the harness's
equivalent) when any watched filesystem drops under 10% or under 5 GB free, or available memory under 10%, whichever
comes first, again when the number keeps falling, on recovery, and if the monitor itself fails. A file update alone is
not a notification. Before relying on it, verify delivery with a harmless synthetic alert, and verify failure detection
by killing a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a
throwaway monitor without killing it and confirm you are notified within two sample intervals; a monitor cannot detect
its own sampling loop hanging. If any of these notifications is unavailable or unverified, say so in the log and check
the status file at least once a minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor or a heartbeat
stale for two sample intervals pauses new launches until monitoring is restored; restart a dead watchdog. An alert is an
instruction to act: stop launching work, remove build output and scratch you own, wait for or stop the job most likely
responsible, and resume only when the watchdog reports headroom. Record the watchdog's handle, watched paths, status
path and delivery mechanism in the log, include its state in every handoff note so a resumed session reconciles or
restarts it, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/get-site-cutover/<nn>-<short-name>`.
- Shape the stack per the outline. Err on the side of bite-sized PRs, without churn: no code added in one PR and deleted
  in a later one. Within this run, a PR that needs correcting is restructured rather than corrected on top.
- Conventional Commits; `feat` and `fix` PRs carry a changelog fragment under `releasing/changelog.d/` in the same
  commit, per root `AGENTS.md` (Releases and the changelog); validate with
  `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; the monitor lands the plan per `plans/AGENTS.md`.
- Apply `.agents/test-authoring.md` to every test change, and the documentation pass from the maintainer's standing
  instructions to every touched file.

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression, through
`scripts/record-test-run.py` for Rust test execution (pinned nextest and tmux per `docs/test-run-evidence.md`). Typical
choices, not a checklist: `cargo fmt --all -- --check`; `cargo clippy --all-targets -- -D warnings`;
`cargo clippy -p farhelm --bins -- -D warnings`; nextest selections of `farhelm-helm`'s provisioning modules
(`release_payloads`, `payloads`, `assets`) and of the provisioning tests that name the release URL;
`cargo check -p farhelm-ui --features desktop` and the desktop nextest selection for the updater's tests;
`sh -n scripts/install.sh && shellcheck scripts/install.sh scripts/test-install-sh.sh`;
`bash scripts/test-install-sh.sh`; `dist generate --check` (pinned cargo-dist, as root `AGENTS.md` says) for PR 3;
`python -B scripts/check-test-sleeps.py` when tests change; `dprint check` on changed files;
`python3 releasing/check-changelog.py format`. No browser tests: nothing here touches the web UI. Say in the report
which checks ran and why.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of that PR's changes to TWO
fresh-context reviewers, as the user demands (M9): a Claude Opus 5.5 agent at high effort, and a gpt-6-astra agent at
high effort (shelled out to the other harness if the executing one cannot reach that model natively). No review swarm.
Both get the same prompt, which carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. This change is security-relevant:
> it decides what installed software trusts, so treat any path by which unsigned or unverified bytes get run or
> installed as a correctness bug. Report findings to the named file, most severe first, each with the file, the quoted
> code, what is wrong, and the smallest fix. Edit nothing and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, its own findings
file (in the scratch directory, one per reviewer), and the acceptance criteria: The goal, Decisions already made, and
the outline above. For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md`
verbatim. Address what both reviewers find before moving on, and log the DECISION where you decline a finding. Do not
write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (a signed metadata format beyond the files listed, a
key management or rotation subsystem, installing in Rust instead of running the signed script, a fallback to GitHub when
the site lacks a release, a compatibility path for pre-switch releases; these are examples, not a blacklist), and
whenever the same component has needed repeated corrective review rounds, run a fresh-context review through
galaxy-brain with this charter, supplying the request, the decisions above, this outline, the current diff and the
proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the ring's representation and the key-file layout, the shape and name of the
public verification entry point, whether the workflow file was renamed, how the installer and helm messages now point
users somewhere, how the updater writes and passes the verified checksums, the changelog fragments' kinds and wording,
the exact TODO.md rewrite, and every reviewer finding you declined.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. A material scope expansion, a weakened
guarantee, or an omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log
entry is not authorization. Anything that would need a decision (for example, if the helm cannot follow the site's
redirect without a change to its HTTP client's redirect policy that loosens HTTPS-only behavior, or if the installer's
test harness cannot express the new layout without a design change): record the concrete tradeoff and block per
`plans/AGENTS.md` (Executing one plan, step 10). Independent PRs of the stack that do not depend on the blocked question
may still be built.

## Done criterion

The plan is complete when its linear stack of draft PRs exists, satisfies the acceptance criteria, has passed the review
gate, and has narrowed the TODO.md entry. Open, not merged. If a `## Decisions` section exists, its latest entry must
also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report
through the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
