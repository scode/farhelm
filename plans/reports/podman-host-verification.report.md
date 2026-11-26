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
