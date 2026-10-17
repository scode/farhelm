# The release risk report

What every release attempt's risk report covers (the commit range, the downgrade check, the ranked risks), when it must
reach the maintainer, and how to split its research across read-only subagents. It is on the stable release, RC release,
dev release and risk-report researchers (fixed prompt) reading lists in `releasing/AGENTS.md`.

Every release attempt, stable, RC or dev, comes with a risk report for the maintainer, delivered without being asked. It
answers two questions: which of the changes going out are most likely to break the release badly, and whether the
release blocks a downgrade. The maintainer asked for it as a default on 2026-10-02, after requesting it by hand for
v0.22.0-rc.1 and v0.22.0-rc.2.

NOTE: The report is research done by reading diffs, not validation. It does not replace the tag's build gate, and it
does not gate an RC or dev tag: start it as soon as the base and version are settled, let it run alongside the bump and
the pre-tag checks, and deliver it when it is ready. A stable release is different only in timing: have the report in
front of the maintainer before pushing the tag (step 7 of `releasing/.agents/stable.md`, "Cutting a stable release"),
since a stable tag reaches every ordinary install. Delivering it is enough: the tag does not wait for a reply.

## Which commits

The report covers what is new since the previous published release of any kind, prereleases included (the same lookup
the RC version default in `releasing/.agents/rc.md`, "Cutting an RC release", uses). A continued candidate therefore
reports only what landed since the last `rc.N`, and a first candidate or a stable release cut right after a stable one
covers everything since that stable release. Release commits live only under their tags, and an RC may have come from an
unmerged stack, so take the range from the merge base rather than from the tag itself:

```
git log --oneline "$(git merge-base vPREV BASE)"..BASE
```

`vPREV` is the previous published release's tag and `BASE` is the commit the release is cut from. Docs-only commits get
a quick `git diff --stat` to confirm they touch nothing shipped, and nothing more.

## Downgrade check

Compare against the last stable release, because that is what an ordinary install falls back to, and also against the
previous published release when that is a different one. Report the numbers either way, including when nothing moved:

```
for rev in vPREV BASE; do
  git show "$rev:crates/farhelm-helm/src/store.rs" | grep 'const SCHEMA_VERSION'
  git show "$rev:crates/farhelm-supervisor/src/store.rs" | grep 'const SCHEMA_VERSION'
  git show "$rev:crates/farhelm-proto/src/lib.rs" | grep 'PROTOCOL_VERSION: u32'
done
```

The two schema versions are what actually block a downgrade. The helm and the supervisor each migrate their database
forward when they open it, and each refuses to open a database whose version is newer than its own ("refusing to open it
rather than risk misreading it"). So once a build with a higher number has run against a state directory, an older build
cannot use that directory, and the half whose number moved cannot be downgraded in place. A protocol version change
blocks nothing on its own, but builds on different protocol versions refuse to connect to each other, so a downgrade has
to take the helm and every host it talks to back together, and hosts left on the other side sit at `needs update`. For a
stable release this is the same protocol question step 1 of `releasing/.agents/stable.md`, "Cutting a stable release",
already asks; answer it once and use it for both.

The constants do not cover every persisted format. Read the diff for anything else an older build would fail to read: a
versioned state file whose version moved, a new variant in an enum that is stored rather than only sent over the wire, a
renamed or relocated file. Say what was checked and what was not.

## Risk review

Rank the changes most likely to significantly break the release: the helm or a supervisor failing to start or refusing
to connect, adding, installing, updating or provisioning hosts breaking, the desktop app failing to start or sign in,
state or sessions lost or corrupted, running agents killed, or the UI wedged across the fleet. Look hardest at code that
runs unconditionally at startup or on every request, at new refusals and validation that could reject input real
installs already rely on, and at anything that changes how different versions of the helm and supervisor interact.
Narrow edge cases with mild failure modes go in the low-risk bucket.

For each ranked risk, give the change and its PR, a concrete failure scenario, what in a real deployment would trigger
it, how likely that seems, and a manual check the maintainer can run on the release to smoke it out. Close with one line
naming the rest as low risk. Write it in the terms of `AGENTS.md`, "Talking to the user": what the user sees, not
internal names.

## Delegating the research

Splitting the commit range across read-only subagents works well and keeps the diffs out of the main context. Tell every
one of them, in so many words, not to run `git checkout`, `git switch`, `git reset`, `git stash`, any `jj` command, or
anything else that moves HEAD, the working tree, the index or refs, and to read other revisions only through `git show`,
`git diff`, `git log` and `git grep <pattern> <rev>`. The release checkout is busy while they work: on 2026-10-02 a
research agent asked only to read ran `git checkout` to look at another revision, swapped the release checkout's files
back to the previous release in the middle of the version-parity build, and that run had to be thrown away and repeated
from `cargo clean`. Verify a delegate's headline claims (a version number, a refusal the review says is new) against the
code before passing them on.

The files every such subagent reads in full first, named in its prompt, are in `releasing/AGENTS.md`, "Risk-report
researchers (fixed prompt)".
