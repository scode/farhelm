# Apt regex fallback can accept package typo

Reviewed commit: 744ea4122d033ef616454fe592ecb2173593736c

## TLDR

A misspelled package may be accepted through apt's pattern fallback instead of failing plainly. The shipped distribution
behavior remains unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location: `crates/farhelm-helm/src/tensorlake/build_script.rs:88-98 at 744ea4122d033ef616454fe592ecb2173593736c`.

Source assessment (read with the later corrections below) (review record g069/g069-d1, event 22):

Reviewed commit: 744ea4122d033ef616454fe592ecb2173593736c. TLDR: A recipe's nonexistent dotted package name can silently
select unrelated matching packages instead of failing for the missing name. A resulting snapshot may contain unintended
software/use more disk, or the billed build may fail on the expanded set. Mechanism: proto recipes.rs:435-464 accepts a
word such as lib. as a package-name-shaped value. Assigned build_script.rs:88-98 shell-quotes and passes the word
unchanged into apt-get -q -y ... install; quoting protects the shell, not apt's selectors. For a cache where lib. is not
a literal package but names containing lib plus another character exist, apt-get's unanchored regex fallback selects
those matches. The example is conditional, not a claim that a particular Tensorlake cache has been inspected. Primary
evidence: https://manpages.debian.org/bookworm/apt/apt-get.8.en.html, install, documents missing-literal regex fallback
and substring matching. Official apt 2.6.1 source https://github.com/Debian/apt/blob/2.6.1/apt-pkg/cacheset.cc#L125-L184
admits a dot when APT::Cmd::Pattern-Only is false, constructs PackageNameMatchesRegEx, iterates cache groups and inserts
every match. :320-329 reaches regex after literal/task/fnmatch lookup fails; :445-475 routes ordinary version arguments
through that selector. Official source retrieved via raw.githubusercontent.com for the same immutable tag.
https://github.com/Debian/apt/blob/2.6.1/apt-private/private-cmndline.cc#L474-L501 enables Pattern-Only specifically for
apt, not apt-get; cacheset defaults it false. Official apt 3.0.3 retains regex acceptance at cacheset.cc:128-140 and
lookup fallback at :323-332. Repository diff of build_script.rs and the validator from reviewed commit through pinned
8a8c9579e04299410652f0e4681975a7198dfc7b is empty, so no later repair in those boundaries. Governing SPEC.md Topology
describes installing the recipe's Ubuntu packages, and SPEC_impl.md Tensorlake recipes describes package-name syntax and
leading-option defense, not expression selection. Build-script tests at :203-231 verify argument quoting, not literal
apt selection. Suggested fix: resolve/encode literal package names explicitly while retaining valid dotted names, arch
and version constraints; rejecting all dots would reject legitimate packages. Suggested bucket other: unintended
fresh/reproducible image contents or build failure, with no established existing-user-work or security-loss route.
Confidence kind: unverified after investigation for the actual Tensorlake failure; static semantics and forwarding are
verified for the inspected apt releases. Exact missing premise: external image apt release/config permits this fallback,
literal selector is absent, matching cache packages are present and solvable. Available pinned repository source names
the external image but contains no apt version/config/cache manifest; immutable primary apt code settles semantics, not
that image's current cache. A retained image/cache manifest or authorized narrow reproduction would settle occurrence;
no runtime/package-manager/cloud experiment was run. This is NEW relative to supplied known g063:l1 removal-suffix
route: regex expansion can add multiple packages with no removal suffix. possible_cover: related g063:l1 shares
selector-boundary motivation and may merit grouping by parent, but its supplied record addresses suffix removal rather
than regex fallback. proposed_drop false. No broader inventory or forbidden coverage search. Earlier d1 lead is fully
resolved into this finding.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g069/g069-d1`; supporting
records `g063/d5`, `g063/g069:g069-d1`. The retained private log preserves earlier wording and its corrections. This
record does not claim runtime reproduction.
