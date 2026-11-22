# The first Delete’s live confirmation reply lacks a regression assertion

Reviewed commit: f2b3b5b9c4be17779dbdce517c1a7af04c9f4eaa

## TLDR

Existing assertions would miss a regression where prompt absence on a first Delete results in an ordinary error instead
of opening Delete anyway.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/tensorlake/recipes.rs:694`.

tensorlake/recipes.rs:680–697 has two distinct ways to issue the marked 409. A durable observation uses the cached
branch at :685–686. A first in-bound attempt gets it only by preserving Podman's typed UnconfirmedImageCapture through
provider aggregation and mapping it at :689–697. The existing helm test environment/podman/failure_tests.rs:1284–1367
forces the first response to be 504 (:1309–1319) and only checks the marker on a later cached retry (:1333–1345). It
waits for the original guard to disappear after absence but never inspects that task’s eventual reply. The GUI test
settings/recipe_leave_tests.rs:333–373 supplies the marker directly, and farhelm-proto/src/ui_channel.rs:1426 checks
mapping metadata. Those assertions can stand even if the freshly returned typed error takes the generic HTTP branch and
the first user sees an ordinary refusal. SPEC.md:509–511 and SPEC_impl.md:6195–6198 at the change (:6204–6207 at tip)
require the follow-up warning after absence. Cover a promptly answered first Delete with no preexisting absence
observation and assert the marked 409 and retained tag. Bucket: other, regression coverage only. Confidence: confirmed
by static inspection of existing assertions, no test execution or runtime observation. Possible coverage: the existing
delayed-absence handler assertion and GUI marker-handling test cover the adjacent halves. This gap is distinct from f6,
which covers a positive lookup on confirmed Delete, and f2, which concerns cached-warning diagnostic precedence.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recipes/f17`.
