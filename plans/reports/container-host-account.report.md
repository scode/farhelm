# Container accounts follow the host where the image permits

## What this was about

Containerized sessions always used the image's uid-1000 account. That kept checkout files owned by the host user, but
the account name and home path could differ from the host and from what a prepared image expected. The agreed behavior
was to adapt to the image in three ordered steps: create the host account when both its name and uid are free; otherwise
use the account owning the host home path in the image; otherwise use the image account with the host uid. Existing
image users are never renamed and their homes are never moved.

The implementation follows that order using image facts collected before mounting the checkout. The agent, terminal tabs
and ordinary container execs inherit the selected account and home, and files they write in the checkout still belong to
the host user. Account preparation is part of creation, so a failed or interrupted preparation cannot expose a partially
prepared container for reuse. The specs and the Containerized sessions docs page explain the behavior, and the completed
TODO entry is removed.

## Things you should know

A stock Ubuntu image still uses `ubuntu` and its home for a host uid of 1000 when the host home path is absent. At a
different uid, a newly created account may have none of the harness installation, configuration or sudo access prepared
only for the old uid-1000 account. Prepare the user's own account, install harnesses system-wide, or seed configuration
through `/etc/skel`. Existing containers keep their original account until Replace; no account setting was added.

A host home owned by root, or by a uid with no image account, does not select that owner: selection continues to the uid
rule. A name collision without a usable home owner or matching uid refuses creation. A free requested gid whose group
name is already occupied also refuses. An existing selected account with a missing image home gets that home prepared
from `/etc/skel`. Home preparation never seeds or changes ownership inside the checkout, even when the checkout is below
the home. The host home is never mounted. Unsupported rootless uid mappings and missing Ubuntu account tools refuse
visibly rather than selecting uid 1000.

Execution found two creation details that needed explicit handling. The local image name is resolved once per creation
so a concurrent retag cannot make inspection and creation use different images. Incomplete preparation remains fenced
off until initialization succeeds or container absence is confirmed; unconfirmed cleanup stays available for Delete or
Replace. These add no stored account state for later execs.

The existing changelog fragment was updated. Its `breaking` classification was preserved because the current fragment
already records the archive-setting retirement from stable 0.32; the plan's earlier assumption that the fragment was
still `added` was stale. This account change uses the agreed `feat:` title without a breaking-change marker.

## Open questions and possible follow-ups

No decision remains open. Account creation currently assumes Ubuntu tools. The focused evidence uses the offline Ubuntu
24.04 image and Podman 4.9.3; it does not establish compatibility with every custom image or unmappable uid layout.

## The PRs

- [#2832: adapt container accounts to the host](https://github.com/scode/farhelm/pull/2832/changes) — one draft PR,
  based on main, head `b96d8183bf210fee4a5ae1bb87fd637ac441ef8b`.

## Checks run, reused and skipped

Only relevant checks were selected. The full test battery was not run.

- Passed: `cargo nextest run -p farhelm-supervisor --lib -E 'test(container::)'`, through the recorder, four slots and
  zero retries: 22 passed, 1361 selected out or ignored. Run `b93bc974-948e-4b3b-9e9b-c3c754a484aa`. This covers the
  account rules and boundaries, command arguments, image retag consistency, initialization readiness, failed cleanup and
  creation fencing.
- Passed: five real-Podman cases selected by
  `cargo nextest run -p farhelm --test e2e -E 'test(containerized_sessions::)'`, recorder
  `4d488e8e-2d8f-4936-a437-b898ae03bc44`, four slots, zero retries and required pinned tmux. Five cases passed, plus the
  fixture-build setup; 404 other cases were selected out. The retained output confirms the user manager was available
  and the dedicated container scope assertion ran. No runtime `SKIPPED` marker was present.
- Passed: recorder-backed Podman probes verified disabled passwd/group injection and inherited user/HOME on Podman 4.9.3
  (`6c1a186e-0ccb-4030-b103-6a76932b13d5`), and the actual initialization script's repair and seeding of an existing
  account's bind-created home while preserving checkout content and metadata (`5ba78190-9983-4a7f-833f-d2f538553e62`).
  Their owned containers were removed and confirmed absent.
- Passed: `cargo fmt --all -- --check`, `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `dprint check` on the five changed Markdown files, `python3 releasing/check-changelog.py format`, and the
  isolated-interpreter `python -B scripts/check-test-sleeps.py` check (327 annotated delays, none unexplained). These
  inspect the changed code, tests and documentation, including the shipped binary configuration without test seams.
- Reused across the commit: the final unit and integration runs and home-repair probe captured base `7272a521` plus the
  final working diff, source fingerprint `23a19486d6c9d2da6aaac047fea9623eff805c4f9683c03b21516067c6718883`. The
  retained raw-diff digest equals the committed PR diff. Source files stayed frozen while checks ran; committing changed
  no tested content. Retained records have complete source capture and output, stream EOF and confirmed recorder
  cleanup. Earlier 20-test and lint passes preceded the corrective fix and are retained as earlier observations, not
  final coverage.
- Skipped: unrelated Rust targets and doctests, browser and JavaScript suites, desktop, installer, provisioning and
  release checks: this change adds no UI, JavaScript, installation or release behavior. No live installation was used.
  Website build and preview were skipped because the incidental docs edit changes prose without changing frontmatter,
  headings, links or generated assets; formatting inspected the changed page.

## Review gate's outcome

The required fresh-context `gpt-6.1-sol` reviewer at high effort reviewed the full change and test-authoring checklist.
One P2 finding was accepted: a checkout mount could manufacture an existing account's missing home before preparation
and incorrectly suppress seeding and ownership repair. The fix carries the home's pre-mount existence fact into
preparation, with two regression tests and a real-Podman probe. The corrective review found no remaining concrete
findings. Creation-scope reassessments found no unnecessary complexity. Commit and PR wording passed the independent
cold read.

### Landing

Landed with a landing fix that marks it as a breaking change (below). New containers for containerized sessions now use
an account matching your host user name, user ID and home where the image permits it, instead of the image's user-1000
account. If neither your name nor your user ID exists in the image, Farhelm creates your account and seeds its home from
`/etc/skel`; if one of them exists and your host home path is a directory owned by a non-root account there, that
account is used with your home; otherwise the existing account with your user ID is used, and when only your user name
exists there, creation is refused. A created account gets your host group ID, reusing a group that already has it.
Existing containers keep their account until replaced.

- #2832: the account choice and setup in the supervisor's container creation, end-to-end container tests, the
  specification text, the Containerized sessions docs page and the changelog entry.

#### What landed since the stack was based, and whether it interacted

Only plan bookkeeping landed since; the plan was based after podman-local-image and a record of a browser test flake
(#2831), and nothing interacted.

#### Fixes made at landing

- The plan treated containerized sessions as having shipped only in a release candidate, so it described the account
  change as a context paragraph inside podman-local-image's changelog entry. But the stable v0.32.0 release shipped
  containerized sessions (marked experimental) with docs asking for images prepared for user and group 1000, so for
  users whose host user ID is not 1000 the next new or replaced container may get an account without that image's
  agents, configuration or `sudo` (when neither their name nor their user ID is in the image). The plan explicitly asked
  for a `feat:` title without `!` and an edit to the existing entry; the landing overrode that, a judgment call you can
  reverse: the change now has its own changelog entry with `kind: breaking`, and the PR and commit are titled `feat!:`.
  The docs page's first account rule (only that one) now says "Farhelm creates your account there" instead of reading as
  an instruction to you.

#### Things you should know

- A security review found nothing that widens what a container can reach: the checkout is still the only mount, the host
  home is not mounted, no privileges are added, account setup runs as the container's root only while the container is
  created, and the user name, user ID and home are passed as separate arguments, never spliced into shell code.
- Host accounts with very large user IDs, common with directory services (LDAP, FreeIPA, Active Directory), cannot be
  mapped by rootless Podman, so creating a container fails for them (large group IDs too) unless the image already has
  their name or user ID and their home path owned by an ordinary account; before this change the fixed user ID 1000
  always worked. The error is Podman's own plus a hint; the docs do not mention it.
- Preparing only a home directory in the image is not enough: the home rule applies only when your name or user ID
  already exists there, and otherwise Farhelm creates a new account and re-owns only the top of that home.
- When an existing account is used with your host home, `HOME` is your host home but the account's own record keeps the
  image's original home, so tools that read the record (ssh, for one) see the other path.
- When the checkout root sits under your host home (`~/src`, say), the directories between the home and the checkout are
  created owned by the container's root, so they are not writable by your account inside it.
- Not covered by end-to-end tests with real Podman: the refusals (only your name existing, a name Ubuntu's account tools
  reject, an unmappable user ID, missing account tools), the image lacking the home of the account with your user ID,
  reuse of an existing group, the home rule when the image's recorded home differs, and cleanup after a failure partway
  through creation.

#### Checks

This round landed only this plan, stacked on main `f8e7a06a`. On the stack as delivered:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, the check that every test delay states its reason
  (`python -B scripts/check-test-sleeps.py`), `dprint check`, `python3 releasing/check-changelog.py format`, the docs
  website build (`bun run build` in `website/`) and the workspace debug build: all passed.
- The whole Rust workspace, `cargo nextest run --workspace --exclude farhelm-desktop`, with the pinned test runner and
  tmux, no retries, on a machine with rootless Podman and the Ubuntu 24.04 image, run
  `4a0d6437-c2bf-43d4-8a19-18b6aada9171`: 4581 passed, 41 skipped by design, 1 failed: the known flake "shallow pause
  resumes without reset or replay", in terminal code this plan does not touch.

After the landing fix (a new changelog entry, one paragraph moved out of another, and one docs sentence):
`dprint check`, `python3 releasing/check-changelog.py format` and the docs website build passed.

Reused: nothing from the executor's runs. Not run: the browser suite and the UI's tests (the plan changes no UI code),
the desktop smoke tests, and the native Mac app (not runnable on Linux; containers are Linux only).

The report's checks were the executor's selected runs; this landing ran the whole Rust workspace, including the plan's
end-to-end container tests against real Podman. Two statements in the executor's report became untrue at landing: that
the existing changelog entry was updated, and that the agreed `feat:` title without a breaking-change marker is used.
Nothing else in it did.
