# Grok attribution skips the pane, so a nested Grok can stand in for an unrecognized session Grok

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

When a Grok session's own Grok runs from a program whose file name Farhelm does not recognize, a second Grok started
inside that session can have its conversation recorded as the session's. Restart then resumes that nested conversation
instead of the one the user was working in. The trigger is very unlikely: it needs a Grok install whose program name
Farhelm does not know, plus a second, differently named Grok run inside the session.

## Details

Found by the 2026-10-22 prioritized persistent review, group `206-harness-non-first-class-2of2` (finding f1).

`grok_corridor` (`crates/farhelm-supervisor/src/procs/grok.rs:30-82`) walks the anchored chain from the reporter (index
0) to the session's pane process (last link). A link whose image `is_grok_image` recognizes (only the basenames `grok`
and `grok-linux-x86_64`, `grok.rs:9-15`) becomes the emitter, and a second recognized link refuses as nested
(`grok.rs:44-46`). Every other link is checked as a hook trampoline, another session runtime, or an unclassified
intermediary, except the reporter and the pane: `if index == 0 || index + 1 == chain.len() { continue; }`
(`grok.rs:62-64`). The pane is accepted by position whatever it is.

This is the same gap triage already fixed for OMP (TRIAGE_OUTCOMES.md `omp-corridor-uncounted-pane-runtime.md` and
`omp-bun-pane-proof.md`): when the session's own runtime sits at the pane but is not counted as a runtime, a nested
runtime below it becomes the only emitter. For Grok the session runtime goes uncounted when its native image has a name
other than the two above. The fixture names (`~/.grok/downloads/grok-linux-x86_64`, `procs.rs` around 2916) suggest the
vendor uses platform-suffixed binaries, so on Linux aarch64 or macOS the image is probably something like
`grok-linux-aarch64` or `grok-darwin-arm64` (unverified).

Triggering chain: the reporter (`farhelm internal hook --vendor grok`), then an optional `sh -c` trampoline, then a
nested Grok whose image is recognized (`grok`, launched with `--no-leader`, run by the session agent's shell tool, where
`bash -c '<single command>'` execs and leaves no shell link), then the session's Grok at the pane with an unrecognized
image name. Grok hooks are global under `$GROK_HOME/hooks` (SPEC_impl.md, Grok hook configuration), and the nested
process inherits the session's credential environment, so its `SessionStart` reaches the session's report slot.
`grok_corridor` returns the nested process as the emitter, and `report_grok_conversation`
(`crates/farhelm-supervisor/src/service/core/vendor/grok.rs:77-211`) admits its UUID through `merge_report` as long as
its timestamp is newer. A session that could never capture before now gets a Resume offer for the wrong conversation,
and the record-pair check passes because the nested conversation's files are real.

SPEC.md "Durability and resume" (around 2245-2247) says Grok reports "must come from one native `grok` process under the
owned pane ... a nested Grok or another session-hosting runtime cannot replace the parent selection", and SPEC_impl.md
"Grok attribution, ordering, and exact-record validation" states the same intent. Neither accepts this gap.

Docs context added by the reviewer's synthesis pass: `website/src/content/docs/docs/agents/grok.md:66-71` says Grok
"works through a wrapper prefix that `exec`s it or stays as its one parent process", that "a renamed binary runs it
normally, but Farhelm cannot follow its conversation, so **restart** stays greyed out", and that it was "Tested on Linux
with Grok Build 1.0.40 and 1.0.50; not yet on a Mac." So accepting a non-Grok pane by position is intentional (resident
wrappers), and the docs promise only lost tracking for a renamed or unrecognized binary, not admission of a nested
conversation. That page is not a cover; it states the outcome this finding contradicts.

How to verify: a unit test like `grok_rejects_nested_and_foreign_runtime_corridors` (`procs.rs` around 2945) with the
last link `corridor_link(11, "/home/user/.grok/downloads/grok-linux-aarch64", &["grok", "--no-leader"])` and a
recognized `grok` emitter below it. `grok_corridor` returns `Ok` today.

Fix sketch: apply OMP's pane rule to Grok while keeping resident wrapper panes working. When the pane is not the
emitter, refuse if the pane looks like a Grok runtime (argv[0] basename `grok`, or an image basename `grok` or
`grok-*`). Alternatively widen `is_grok_image` to the vendor's platform-suffixed names, so a nested Grok under such a
pane counts as a second emitter. Either way, add a unit test for the uncounted-pane chain.

Suggested bucket: highest by consequence (SPEC.md "First-class harnesses" keeps resuming the wrong conversation in scope
for non-first-class harnesses), with negligible likelihood, matching the OMP precedents' assessment.

Confidence: likely. The code path is confirmed by reading. Unverified premises: Grok's native image on some supported
platform has a basename other than `grok` or `grok-linux-x86_64`; and a second Grok whose image is recognized (a
different install) runs nested in that session with `--no-leader` and fires `SessionStart` with source `new` or `load`,
which needs two differently named Grok installs on one host.

Reviewer's proposed drop, kept for triage: maybe drop as negligible likelihood. FILTER.md "Rare edge cases in harnesses
without first-class support" does not match, because it excludes "resuming the wrong conversation".

Possible cover: none found. Related, not a cover: TODO.md "Maybe later", "Keep a nested Grok from erasing the session's
conversation switch" (around 707-729), which asserts "Farhelm never records the nested `grok`'s conversation; the
failure is a stale or missing conversation, never a wrong one". This finding is a case where that assertion fails.
