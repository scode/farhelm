## What this was about

Six triaged bugs concerned documentation and the maintainer's README screenshot and demo video tooling. The uninstall
guide promised protection for modified Mac app files that uninstall does not provide; smaller documentation headings
were hard to read in light mode; and the local desktop build recipe could split outputs when given a relative build
directory. Both capture scripts could build successfully in a custom directory while launching older checkout-local
outputs. The hero publisher's self-test assumed two identical publishes would produce different commits, which could
fail when their timestamps matched.

The maintainer asked for these small fixes together in one draft PR, with each outcome left for triage if it needed
substantially more complexity. All six were confirmed and fixed within that scope.

## Things you should know

The uninstall guide now distinguishes Linux content checks from the Mac app's record, layout, file-type and ownership
checks. It explicitly says that modified recognized Mac app files can still be removed. Uninstall behavior itself is
unchanged.

Light-mode smaller headings now use a darker foreground; the measured contrast in the built CSS is 8.41:1 against the
light page background. The local desktop build recipe exports its normalized build directory before either build.

Both screenshot and video capture refuse an explicit build directory outside the checkout's resolved `target/` before
builds or output changes, including with `--no-build`. An explicit checkout-local directory is accepted even before the
first build creates it, and is exported as an absolute path. The capture tools still launch checkout-local outputs.

The publisher self-test now uses different image content for its second publish, so differing commit hashes do not
depend on timestamps. Its Git redirect configuration is passed only to Git children, without changing the test process's
environment. No real capture, hero publication, website deployment or installation mutation was performed.

All six execution entries point to the same change and draft PR, and their feedback files and index entries are removed.
No outcome was discarded or left unfixed.

## Open questions and possible follow-ups

None. No product or design decision was needed, and no complexity gate tripped.

## PRs

- [#1809 — documentation guidance and capture reliability](https://github.com/scode/farhelm/pull/1809/changes), draft,
  based on main.

## Checks run, reused and skipped

- Passed: `bash -n` and `shellcheck` for the three changed shell scripts, changed Markdown `dprint check`, and
  `python3 releasing/check-changelog.py format`. Markdown and fragment formatting were checked again after rebasing and
  adding the PR URL.
- Passed: `cd website && bun install --frozen-lockfile && bun run build`, including internal links; built CSS contrast
  was checked against the plan's 4.5:1 minimum.
- Passed through the test recorder, run `fb0c8d5a-1570-4815-a377-07b0c0aa998c`: outside-target refusals in build and
  no-build modes occur before spy tools or output sentinel changes; accepted relative targets are exported absolutely;
  the documented desktop recipe passes that directory to its build child; and the publisher self-test passes with
  identical injected Git timestamps. A mutation restoring identical second-image content failed the expected replacement
  assertion.
- Passed through the test recorder, run `a63bcfab-5ad5-47ab-94fb-f0b77723729f`: copies of the actual capture scripts in
  an owned fixture checkout accept absent relative and absolute local target directories without creating them, and
  refuse absent outside targets before tool invocation.
- Reused the website build and runtime proofs from the work based on `058247cb`. The careful rebase onto `499d6160`
  added host-text escaping, Git environment isolation, OS readback fixes, harness fixes, atomic SSH configuration writes
  and plan bookkeeping. Their complete implementation diffs were inspected; they do not change this plan's scripts, CSS,
  build recipe or Mac content-check contract. The rebase preserved both sets of bookkeeping edits and introduced no
  source correction.
- Skipped Rust, browser, desktop runtime and installer suites: this plan changes documentation, website styling and
  capture tooling, with their concrete risks covered above. The test-sleep check does not apply because no Rust or
  browser tests or helpers changed. Full captures and real publication were excluded by the plan.

## Review gate outcome

The prescribed fresh gpt-6.1-sol high source reviewer found one regression in the first guard: it refused a valid
checkout-local target before that directory existed. The guard was corrected in both scripts and demonstrated in the
second recorded proof. The reviewer then reported no remaining findings, including against the Mac uninstall record and
the full test-authoring contract. The reviewer inspected source; the executing session verified runtime checks
separately.

The final fresh gpt-6.1-sol medium wording reader understood the problems and refusal rationale and found the claims
accurate and conventions satisfied. An earlier reader violated the blind-read ordering and was interrupted; that report
was not accepted. Requested native model identities and usage counters were not independently exposed. Implementation
remained local under no-workhorse mode. The executor left the PR in draft and did not merge it.

### Landing

Landed on 2026-10-10 (UTC) as #1809, one squash commit on main. Since the plan was based, main gained this day's other
landings; none touches the files it changes, and the rebase was clean.

#### Review before merging

A separate reviewer read the change by reading the files only and found all six triage outcomes carried out as their
ledger entries require. The capture scripts' new target-directory guard compares resolved paths and runs before any
build or deletion; the publisher's self-test now uses an inverted second image so its replacement check no longer
depends on timestamps, and no longer exports variables into its own process; the uninstall guide's claims match the
uninstaller's checks; the light-theme heading color is about 8.4:1 against its background.

#### A fix made while landing

The desktop build recipe in `crates/farhelm-desktop/README.md` now exported `CARGO_TARGET_DIR` into the shell it was
pasted into, so later builds of other projects in that shell would land in Farhelm's target directory. The landing
wrapped the recipe in a subshell and said why. A one-paragraph line-wrapping mismatch that change introduced was fixed
in a follow-up commit right after the merge.

Smaller notes left as they are: the uninstall guide now names the checked file types and ownership twice in a row; the
changelog fragment's "uninstall guide" is a repository document no user-facing page links to; the light-theme override
of the shared heading color would also recolor an unused panel component's border; and the two capture scripts carry the
same target-directory guard, which, like the docs screenshot script's, checks only `CARGO_TARGET_DIR`.

#### Checks

- Run now: `shellcheck` on the three scripts, the README hero publisher's `--self-test` (its whole validation), the
  changelog lint, and the website build (32 pages, internal links).
- Reused from the executor: its recorded self-test run with a deliberate failing mutation, and its script checks.

Nothing in the report above was made untrue by the landing.
