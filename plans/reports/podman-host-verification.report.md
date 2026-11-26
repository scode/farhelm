# Report: verify container host identity with cheap sanity checks

## What this was about

A container host is an ordinary local or SSH host that runs Podman containers for Farhelm sessions and recipe builds.
Farhelm records which machine and Unix account a container host is: its claim. Before running a command there, it checks
that the SSH destination still reaches that machine and account. The risk is an alias that someone re-points (in
`~/.ssh/config`, say): without the check, Farhelm would run container commands, including deletes, on whatever machine
the name now reaches.

The TODO entry listed gaps in that check:

- The host "proved" itself by printing back a random token that Farhelm had just sent it, which anything that repeats
  its input can do.
- Several paths did no check at all:
  - container setup scripts, which carry private input such as proxy settings;
  - the proxy read when a new session container is set up;
  - the Containers list's leftover listing and Delete. Their first look at a host could even replace the claim they were
    meant to clean up under.
- An existing container's commands reused the last check as long as the destination's text was unchanged, even if it now
  reached another machine.

The maintainer narrowed the TODO's much larger ask to cheap sanity checks, in two sentences the plan quotes:

- SPEC.md: editing `~/.ssh/config` to re-alias or re-point hosts while Farhelm operates is undefined behavior.
- SPEC_impl.md: where cheap, Farhelm applies basic sanity checks, without significant complexity to defend against it.

The plan also ruled out four things:

- holding back private input until the host has proved itself;
- redesigning how files are streamed to the host;
- merging the host check's separate commands into one;
- a second cache of checks per identity.

What the PR does:

- **The host computes its proof.** Before running anything, the host compares a digest of its machine ID and account
  with the one Farhelm expects. A host whose machine or account differs stops there, before the command runs. A matching
  host prints a second digest that only a host with that machine ID can compute. Farhelm checks that digest before it
  trusts the output. Repeating Farhelm's input no longer passes.
- **The unchecked paths now check.** Container setup scripts, the creation-time proxy read and the proxy installation
  steps all go through the same check.
- **An existing container's commands compare identities.** When the last check saw another machine than the one a
  container was made on, Farhelm checks the host again once. If it still differs, the command stops with "Farhelm could
  not verify this container host's machine and account. Its SSH destination may have changed outside Farhelm. Restore
  the original destination, then try again." Once the destination is restored, the next command checks again and the
  existing containers work again. If the host's identity cannot be read at all (missing `sha256sum` or `tr`), that
  reason is shown instead.
- **Leftover listing and Delete read the claim first.** They read the stored claim before anything looks at the host,
  and bind every command to it.
- The two policy sentences are in SPEC.md and SPEC_impl.md, the TODO entry is removed, and a changelog fragment is
  added.

## Things you should know

- **Leftover cleanup's binding to the stored claim holds for one operation only.** A leftover listing or Delete that
  refuses because the host moved still checks the host once, as every operation does. If that host has no containers,
  images or recipe build helpers recorded, the check may replace the stored claim with the moved host's. A retry after
  that is bound to the new claim and acts on the machine the alias now reaches (only on a container with the same ID;
  see (c) below). The TODO asked for the cleanup identity to be kept across that refresh and a retry. The plan's
  narrowed scope stops at "read the claim before the first look", so this remains. Keeping it would mean holding the
  claim across operations, or refusing to replace a claim while the host disagrees.
- **Private input is still sent before the proof.** On a wrong host, a setup script's private input (proxy settings,
  which can contain a password) is sent, though no command there reads it, because the check stops first. SPEC_impl.md
  now says so explicitly.
- **The TODO cross-reference changed.** Another TODO entry, "Show container recipe admission failures before recording
  build ownership", referred to the removed entry for its shared rules. It now points to SPEC_impl.md's "Podman places,
  creation and images" section instead. That entry's plan (podman-recipe-admission) waits on this one.
- No test ran against a real SSH host or real Podman. The proof itself is tested in a real local shell:
  - an honest wrong account exits before the command runs;
  - a shell that echoes Farhelm's input and runs the command is rejected.

  The rest is covered with the helm's scripted Podman fakes.

## Open questions and possible follow-ups

- Whether to keep the cleanup identity across a refused operation and its retry (first item above). The options:
  - (a) Hold the claim across a refused operation and its retry. This needs a new mechanism to remember it.
  - (b) Refuse to replace a claim while the host disagrees with it. No new mechanism, but a user who moved the host on
    purpose must remove and re-add it.
  - (c) Leave it, under the "undefined behavior" policy. A retried leftover Delete after a refused one may act on the
    machine the alias now reaches, but only on a container with the same 64-character ID there.

  I recommend (c), unless leftover cleanup on re-pointed hosts turns out to matter.

## PRs

- #5084 — fix: require a place proof the container host must compute.

## Checks

Run now, on the PR's commit:

- `cargo nextest run -p farhelm-helm --lib` on the Podman, container, place and leftover tests plus `environment::` and
  `access::`: 185 passed (recorded run a3a749b4-29e1-4a6d-994e-dde8198f44e8). It includes three new or extended tests:
  - the real-shell proof test now also covers a forged echo;
  - a cached check of another identity triggers exactly one recheck, then refuses, and recovers once the destination is
    restored;
  - leftover listing and Delete, each run cold against a moved host, refuse without sending any Podman command.
- `cargo clippy -p farhelm-helm --all-targets -- -D warnings` and `cargo fmt -p farhelm-helm -- --check`: clean.
- `dprint check` on SPEC.md, SPEC_impl.md and TODO.md, and `python3 releasing/check-changelog.py format`: passed.

Reused: none.

Skipped:

- The real Podman end-to-end suites: they need a prepared rootless Podman host. The change is in the helm's command
  layer, which the unit tests above drive through the production wrapper and the scripted engine.
- The workspace battery and the desktop app's tests: nothing outside the helm's container code changed.

## Review gate

A single fresh-context Opus 5.5 reviewer at high effort, using the plan's general charter and the test-authoring
checklist, reviewed the PR in two rounds.

Round 1 found one medium-severity problem and five low ones. All were fixed:

- **Medium:** setup scripts replaced the "destination may have changed" reason with a generic "failed or timed out". The
  reason now gets through.
- A host whose identity could not be read was reported as moved. It now gets its own reason.
- Container run and start still skipped the identity comparison. They now use it.
- Two doc comments and one test assertion were fixed. The assertion was loose enough to pass on the wrong failure.

Round 2 found nothing.

### Landing

#### Two safeguards

These notes use two terms. The **host proof** is the new check the host itself runs before each wrapped command: it must
show it is the recorded machine and account, or the command does not run. The **container comparison** is what commands
for an existing container or recipe build do first: they check that the last host check saw the same machine the
container was made on, and check the host again once if not.

#### What landed with it

This round landed this plan, then supervisor-outliving-programs (Stop and Restart ending an agent that outlived its
killed terminal). The two touch separate code and separate parts of the specs. Since this plan's base, main had gained
several app-side plans and two helm changes unrelated to containers (each custom agent remembering its own wrapper,
#5073 and #5074); while it landed, a TODO entry about letting agents start YOLO sessions on added hosts (#5089). An
independent reviewer and I checked them against this plan: none calls or changes the container host code, so nothing
interacted.

The reviewer and I both read the host proof. It is plain POSIX shell and needs nothing the host check did not already
need (`sha256sum`, `tr`). What the host hashes matches what the helm computes. A host whose machine or account differs
stops before the command runs, and a shell that echoes Farhelm's input back is rejected. None of the proof's own
commands reads input, so a setup script's input reaches the container intact. Neither the executor's checks nor its
tests used a real host, so I ran the real Podman scenario on the test worker (below). It passed, with every container
setup script and recipe build command going through the host proof on a real host.

#### Spec corrected to match the code

The implementation spec said every step for an existing container or recipe build makes the container comparison. Three
steps do not: setting up where a new session container installs Farhelm's own tools, and a recipe build helper's
preparation of its service manager and of its images. They also do not use the host proof, since installing files on the
host was left out of the plan's scope. So if a host's SSH name was re-pointed to another machine and the host was
checked again in between, these three steps would act on the machine the name now reaches. That is the "undefined
behavior" the plan's policy accepts. The spec now names the three; no code changed.

#### Worth knowing

- **Follow-up: add the container comparison to the three steps above.** About five lines, passing an identity the code
  already has at hand; no redesign. I left it rather than change behavior during the landing.
- **Two claims in the report are too broad.** It says the proxy installation steps all go through the same check: the
  creation-time proxy read uses the host proof, but the upload of the proxy settings file (which can hold a password)
  gets only the container comparison, as the plan's Decisions allowed (no redesign of how files are sent to the host).
  On a re-pointed host that comparison stops the upload only if the last host check already saw the other machine. It
  also says an existing container's commands compare identities; the three steps above do not.
- **No test would notice if a path stopped using the host proof.** The tests' stand-in Podman accepts commands with or
  without the proof around them, so a later change that dropped it from setup scripts or the proxy read would still
  pass. A follow-up could make the stand-in refuse commands without it.
- **A persistently moved host is checked in full every time.** When a container's host now answers as another machine,
  every command or connection attempt re-runs the whole host check (several SSH commands) before refusing, rather than
  failing from the last result. The plan asked for one fresh check per command; it only costs much if the mismatch
  persists, for example after the machine was reinstalled.
- **Leftover listing and Delete on a host with no stored claim yet** run without comparing against any recorded machine,
  as before, since nothing is recorded yet. Not a regression.
- On the report's open question about keeping the cleanup identity across a refused leftover operation and its retry, I
  agree with its recommendation (c): leave it, under the "undefined behavior" policy.

#### Checks

Run once for the whole round, on podman-host-verification and supervisor-outliving-programs stacked in that order on
main as it stood when the landing started; what main gained since each plan's base is in each plan's notes. The test
worker is an aarch64 Linux machine with rootless Podman and a systemd user manager:

- On the test worker, `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`:
  clean.
- On the test worker, `cargo nextest run -p farhelm-helm -p farhelm-supervisor`, recorded run
  43812fa9-6ae7-42ce-8ff5-3cd2a81d4035: 3,606 passed, 12 skipped (tests marked ignored). That is every helm and
  supervisor test, wider than either executor's selection.
- On the test worker, the end-to-end tests of host terminals, relaunches, Restart and Restart with, and the stand-in
  Podman modules (`cargo nextest run -p farhelm --test e2e` filtered to `host_terminals`, `host_relaunches`,
  `restart_with_resume`, `scoped_podman`, `containerized_sessions` and every test whose name contains `restart`),
  recorded run 61da3f76-4294-4420-a2d7-6012a29975ec: 106 passed, and none skipped for lack of systemd or SSH.
- On the test worker, the real Podman environment scenario
  (`cargo nextest run -p farhelm --test e2e -E 'test(/^podman_environments::/)'`), recorded run
  ee60cfb9-8dbf-46ae-a67c-bcc4a22c245c: passed, and it did not skip. It runs the production Podman provider against a
  real rootless Podman on the worker, so every container setup script and recipe build command went through the new host
  proof on a real host. Neither executor ran it.
- Locally, `cargo fmt --all -- --check`, the test-sleep check, `python3 releasing/check-changelog.py format` and
  `dprint check` on the changed Markdown: passed, and `dprint check` on the implementation spec again after the landing
  fix.

Reused: nothing. Skipped: the desktop app's tests, the screenshot comparison and the desktop app's end-to-end smoke
test, since nothing in the app changed; the real SSH variant of the Podman scenario, which needs a host set up for it.
