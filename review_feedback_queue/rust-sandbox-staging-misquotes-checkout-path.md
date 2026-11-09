# Rust sandbox staging splits checkout paths containing spaces

Reviewed commit: f17458c5ed6159cac9f621cc2350360d00fa396c

## TLDR

The Rust sandbox runner cannot stage an ordinary checkout whose absolute path contains spaces, because nested remote
shell commands interpolate that path without quoting it.

## Details

Suggested bucket: **other**. Recorder confidence: **verified**. The original reviewer's confidence, qualifications and
severity history are retained below; source-confirmed conditional behavior is not a runtime observation.

The catalog file-URL item concerns # and percent escapes in browser-runner module loading. This separate Rust
remote-shell boundary fails on ordinary whitespace paths; no accepted path restriction or transient retry route was
found. No hostile-account security claim is made.

## Recorded source assessment

Reviewed commit f17458c5ed6159cac9f621cc2350360d00fa396c. TLDR: the Rust sandbox runner cannot stage a normal checkout
whose absolute path contains a space. Checked numbered rust-tests.ts:28-80,116-148 and all checkout/space/quoting
references in .agents/tensorlake.md and both Tensorlake entrypoints. REPO is Deno.realPathSync of this checkout (:34) or
the explicit --repo (:123), without a path grammar restriction. Direct dax argv interpolations in pack safely preserve
path values, but stage :64 and :70 interpolate REPO into nested remote sh -c source without shell quoting. For
REPO=/home/user/work trees/farhelm, sudo install receives two directory operands and cd receives multiple words (or
changes to the wrong prefix under a shell permitting it); the intended full checkout directory is never
established/unpacked. The source path preservation is essential to the compiled-in fixture paths, so staging elsewhere
is not equivalent. Suggested bucket other: maintainer tooling portability/correctness. Confidence confirmed from shell
parsing; no runtime/cloud action. proposed_drop false. possible_cover none checked; SPEC_impl Testing single-user
assumptions accept hostile-account exposure but do not accept ordinary whitespace path breakage. No hostile-local-user
security consequence asserted. Fix: quote the path for the remote shell, or pass it as a positional argument to fixed
shell source / use direct argv for install and an explicit working-directory mechanism for unpack.

Provenance: trailing-review run `f57167f6-e8b1-4ac6-ac31-413e21137114`, original record `g045/l7`, exclusive base
`49ce8cb76e9f4c841e15122a89264c7c908171dc`, inclusive tip `9e3ab60b86025ca74948ededd09b99cd81a2477c`. The private
findings history, dispositions, delivery audits and independent verdicts preserve revisions and narrow exclusions. No
fix or triage decision was executed.
