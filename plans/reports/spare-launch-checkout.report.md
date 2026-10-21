# Prepared checkouts for faster launches

## What this was about

Starting a session in a new managed checkout repeated several seconds of local Git work even when nothing changed on
GitHub. The maintainer chose to keep one earlier clone ready per repository and host, accepting its extra disk use and
the differences from a clone made at launch. The stack also makes Git consolidate repository caches sooner, which helps
ordinary clones independently of prepared checkouts.

A launch can now update that earlier clone and move its contents into its newly allocated directory, preserving the
directory's existing identity. The post-clone command and agent still run at launch in the final directory. The
supervisor prepares the replacement after the launch's clone stage ends. An unusable or absent spare uses the ordinary
clone path; a failed launch-time fetch stops visibly rather than asking for credentials again through a fallback clone.

## Things you should know

**keep a checkout ready for faster launches** is on by default under Settings → checkouts, including after an upgrade.
It uses one extra checkout's disk space per repository and host. The earlier clone keeps older modification times on
unchanged files, records its earlier clone and update in Git's reflog, and can retain objects deleted upstream until
Git's cleanup. Git hooks and checkout filters run during background preparation in another directory, then for the
update in the final directory; that update uses `post-merge` rather than `post-checkout`.

Background preparation uses the user's login shell and Git configuration, with no terminal or credential prompts, low
CPU priority and a roughly ten-minute deadline. It does not run Farhelm's post-clone command or an agent. Failure leaves
future launches able to clone normally. The timeout cannot force a stuck filesystem operation to return or contain
programs that deliberately detach themselves.

Turning the switch off affects future creates. Each repository's spare is discarded at its next create on that host, or
with its cache after 30 days unused. Choosing another checkout directory discards the old spare at the next create.
Accepted requests and their retries keep their original choice. Old retained requests without this field remain off.
Farhelm owns `.farhelm-spares` inside the checkout directory; uninstall retains that data.

Git 2.48 and later notices a renamed default branch and falls back to a normal clone. Older capable Git can retain the
original default branch while it still exists upstream. Git below 2.17 uses the ordinary clone path. Deleted upstream
branches are detected on every take-capable version.

The Start a session guide and both uninstall pages explain the behavior. No speedup beyond avoiding the repeated local
clone work is claimed from these tests. The PRs remain drafts for the landing monitor.

## Open questions and possible follow-ups

No maintainer decision is required to land this plan. Native macOS execution was not available for this run; browser
WebKit coverage does not test the desktop shell or macOS filesystem behavior.

## The PRs

- [#2508](https://github.com/scode/farhelm/pull/2508/changes): consolidate repository caches sooner.
- [#2509](https://github.com/scode/farhelm/pull/2509/changes): retain spare ownership and clean up confined leftovers.
- [#2510](https://github.com/scode/farhelm/pull/2510/changes): take and update a prepared checkout at launch.
- [#2511](https://github.com/scode/farhelm/pull/2511/changes): prepare replacement checkouts in the background.
- [#2512](https://github.com/scode/farhelm/pull/2512/changes): enable the default-on setting and document its tradeoffs.

## Checks run, reused and skipped

Final-setting validation ran against the reviewed final unit above `70b63b80`, with its full source fingerprints
retained by the recorder:

- Selected helm/supervisor unit tests using `cargo nextest run -p farhelm-helm -p farhelm-supervisor --lib`: 66 passed
  in run `4efc7ffc-a33f-46ee-bb47-04578de49105`. The selection covers checkout configuration, schema migrations,
  default-on upgrade and saved opt-out, protected saves after caller cancellation, fresh resolution and retained-create
  fingerprints/recovery. The only later Rust change removed an obsolete comment.
- `cargo nextest run -p farhelm --test e2e -E 'test(github_checkouts::)'`: all five actual-stack cases passed in run
  `a5aa0525-2cee-4078-919b-46973482fba6`, including default-on clone, actual reuse, replenishment, frozen lost-reply
  replay after a helm restart, and the next opted-out create. Its JUnit report also counts one successful fixture-build
  setup entry; that is not a sixth product test.
- Selected browser command over `github-checkouts.spec.ts`, `settings.spec.ts` and `tooltip-coverage.spec.ts`: 72
  passed, 36 each on Chromium and WebKit, in `8d4a44a5-6718-4b5b-8e17-3380af69fd03`. This includes the whole checkout
  spec, the new setting, affected Settings navigation/geometry/dismissal/save cases and sidebar tooltips. The recorder
  retained complete source identity, report and untruncated output, with no runtime skip, retry or flaky outcome. Its
  source fingerprint was `8b98d4a58a480e9317068a35c09ce99c2fec29134c9b46b4230736312f38cfb5` above `70b63b80`.
- `cargo build`, the release web build, affected-package all-targets Clippy and shipped-binary Clippy with test seams
  off passed. Rust formatting, dprint, the isolated test-sleep check (314 delays, zero unannotated), changelog-fragment
  format and the website build passed. All website internal links and anchors resolved.

Earlier review-unit evidence remains applicable to the unchanged preparation and cleanup mechanisms:

- The initial cache clone/concurrency selection passed seven cases in `5994a292-9165-4710-8738-d7ca6fb5bac8`, on PR1's
  reviewed content above `2f0bfad6`. PR4's shared-clone tests subsequently covered the same commands.
- The ownership, allocation, discovery, cache and confined-removal selection passed 100 cases in
  `661ab0b9-e120-49f8-bd1d-8fd579728f4c`, on PR2's reviewed content above `21c98057`. After review corrections, all 14
  spare-storage cases passed in `99a75b97-3549-489f-b1f9-2bb1a1b24598`; PR4's later selection reran the spare storage
  and cache paths. The allocation/discovery/removal contracts have not changed since those checks; promoting the
  opened-entry observation helper for the take was mechanical.
- PR4's reviewed content above `ceb1de43`, fingerprint
  `6569dbe7d9024907ac0bff5c756536c8eb45a90bcee9b8faf069f05af5d561af`: 160 selected supervisor/protocol tests passed in
  `e717d185-b751-48b7-b6dc-7b07af0821ff`. The selection includes the visible clone path, cache, spare ownership, take,
  producer scheduling, cancellation, publication and frozen encoding. Enabling the helm setting does not change those
  mechanisms.
- The actual builder CLI target (`cargo nextest run -p farhelm --test spare_builder`) passed in
  `0b61e470-04a6-44b2-992b-7adc87620b07`. Ten cases exercise cloning, missing interactive credentials, guard lifetime,
  deadlines, owner death and leftover descendants; the eleventh is the disposable-owner entry point. After test cleanup
  changed, both affected owner-death cases passed again in `3caaf7c2-b51c-495c-96fa-c5861b9057b4`. No later producer
  implementation changed.
- The take selection for unchanged/update/deleted/default-branch cases passed four cases each with actual Git 2.17 and
  2.48 in `685ffb76-e2a6-4c1a-aaa7-3c7bfbbbd039` and `6f4cc7b8-f51a-46bd-a9c2-8ce5876d03be`, on PR3's reviewed content
  above `c0c24763`. The later clone-core extraction preserved these take commands and was also covered by PR4's launch
  tests. No public Git installation was replaced.

Failed development attempts remain failed observations. Parser and fixture mistakes found during the earlier units were
corrected and checked narrowly before their passing gates. The first final-unit browser run failed on obsolete checkbox
counts and a test action whose postcondition contradicted immediate save-refusal rollback. Those were test corrections;
the rollback itself was visible and correct. A mistyped exact-test selection executed no tests. Retained failure records
and their dispositions are in the plan's working log. None of these same-session defects was logged as a latent flake,
and no failed command is counted as a pass.

The full stack was carefully rebased onto main at `e7d3bb10`. Incoming main changes were read in full: the only
executable change concerned the independent host-update warning and its browser fixtures; the rest was plan/review
bookkeeping and an unrelated TODO removal. The rebase kept both TODO removals. Each of the five review units has an
empty interdiff against its tested, reviewed version. No checkout, setting, lifecycle or Git behavior changed, so the
successful commands above were reused rather than repeated after the rebase. These are tests of the preserved content,
not fresh executions on the rebased commit ids.

The full Rust and browser batteries, unrelated JavaScript and installer tests, desktop smoke, native macOS execution and
CI were skipped. The selected checks cover the changed configuration, lifecycle, Git and browser boundaries; installer
and desktop-shell mechanisms did not change. WebKit covers the browser engine family, not native macOS. No executable
documentation example or doctest changed. CI is manual and was not requested.

## Review gate outcome

Each PR received one fresh-context general review from `gpt-6.1-sol` at high effort. Findings were corrected within
their review units and checked narrowly by the same reviewer. The final review found only an obsolete staging comment;
it was removed and verified. No finding remains open. Fresh scope reviews removed unnecessary per-temporary-tree records
and scheduling history while retaining the required cleanup and lifecycle guarantees. Commit and PR wording passed
separate cold reads.
