# Image TMPDIR can stage skeleton files inside the checkout

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

An image-selected temporary directory can put account-preparation scratch files inside the checkout despite the stated
exclusion.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The retained source route concerns newly allocated skeleton staging and metadata changes. Independent review narrowed
the earlier broad zero-modification claim: it did not establish overwriting/chowning pre-existing checkout contents or
an arbitrary concurrent replacement route. No image or filesystem experiment ran.

Source locations: `crates/farhelm-supervisor/src/container/account.rs:288-291`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Synthesis of f2, reviewed d6645f771bee8ed03a66486ef8755834937d759d, introduced f73589c5201a0408f2f1555c18a4efca6f6057be.
Retain as other-bucket contract mismatch with bounded impact. Image TMPDIR is a supported environment input: Podman
v4.9.3 pkg/specgen/generate/container.go120-128 merges image environment, and libpod/oci_conmon_exec_common.go673-687
copies the container process environment into root initialization exec despite its user override. Farhelm
account.rs378-399 does not override TMPDIR. GNU mktemp manual13,29-34 confirms mktemp -d uses TMPDIR. A prepared image
can point TMPDIR at an existing container-root-writable tmp directory in the checkout (including a relative tmp path
under the inherited checkout workdir). Home initialization then allocates its scratch in that bind. The actual
boundaries matter: mktemp exclusively creates a new directory; cp -a at290 writes into that newly allocated tree; chown
-hR at291 recursively re-owns that tree, not the preexisting TMPDIR directory; the EXIT trap at289 normally removes only
that tree. The direct home and final merge exclusions at273-308 do not cover the scratch allocation, but this is not an
overwrite or re-ownership route for preexisting checkout files. No evidence of security crossing, credential loss or
user-work loss was established. SPEC_impl.md5811-5813 explicitly says preparation never modifies anything inside the
bind, so even successful temporary staging contradicts that statement; SPEC.md3602-3604's narrower wording about seeding
may be read as focused on the permanent home destination, which is a scope question for parent/triage rather than an
accepted staging exception. Tests cover default staging and record command arguments but no image-directed TMPDIR.
Confidence confirmed conditional source behavior; no image occurrence or runtime experiment claimed. proposed_drop none;
possible_cover only the wording-scope question just described, with no explicit acceptance found. Prior coverage checks
still apply: no matching Planned item, unlanded plan, BUGS, queue item or triage decision; the host installer TMPDIR
exception has different trigger and scope. The initial permission-refusal variant is not claimed. Suggested correction
is to choose and validate a staging parent outside the bind before allocating scratch, leaving the later agent's image
environment intact.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g125/f2`, `a006/g125-f2`, `a032/f1`. These are
original reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts
remain in the private review evidence. No fix or triage decision was executed.
