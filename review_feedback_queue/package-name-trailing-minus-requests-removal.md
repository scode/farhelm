# Package name trailing minus requests removal

Reviewed commit: 68790603037bb51ccd8655e4a948c7ffb747760a

## TLDR

A recipe package ending in a minus sign is passed to apt with its removal meaning instead of being rejected or treated
as a literal package name.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-proto/src/tensorlake/recipes.rs:332-336 at 68790603037bb51ccd8655e4a948c7ffb747760a`.

Source assessment (read with the later corrections below) (review record g063/l1, event 9):

Reviewed commit: 68790603037bb51ccd8655e4a948c7ffb747760a. TLDR: A trailing '-' package entry can request removal in a
recipe's fresh build sandbox, yielding an unexpected reproducible image or a failed build/retry instead of installing
the named package. Actual image-specific damage is not established. Mechanism: package_word_is_valid, recipes.rs:320-350
(name_ok:332-336), accepts openssh-server- as one package word. The consumer first arrives at
744ea4122d033ef616454fe592ecb2173593736c: crates/farhelm-helm/src/tensorlake/build_script.rs:88-98 shell-quotes each
word, joins it into apt_get install, and forwards arguments with quoted $@ to apt-get. Quoting preserves one argv word
but does not disable apt's package-selector syntax. Primary docs:
https://manpages.debian.org/bookworm/apt/apt-get.8.en.html, install, states that an appended hyphen requests removal and
a plus requests installation. Primary source, apt 2.6.1:
https://github.com/Debian/apt/blob/2.6.1/apt-private/private-install.cc#L570-L599 defaults to MOD_INSTALL, registers '+'
as MOD_INSTALL and '-' as MOD_REMOVE, and groups supplied words; #L632-L643 runs the resulting removal group.
https://github.com/Debian/apt/blob/2.6.1/apt-pkg/cacheset.cc#L362-L405 detects the postfix, tries the full literal input
through FromString(..., onlyFromName=true), restores the default action if that succeeds, otherwise resolves the
suffix-stripped word with the selected action. Therefore the concrete suffix route requires that the full spelling (e.g.
openssh-server-) does not resolve literally while its base (openssh-server) does. Valid literal package names such as
g++ must not be described as injection or indiscriminately rejected. private-install.cc:1034-1067 marks an installed
package for deletion; an uninstalled one is left uninstalled. The actual Tensorlake image cache and package presence
were not inspected, so the example's precise downstream removal/failure remains conditional; apt semantics and the
static consumer route are confirmed. Cross-group reconciliation: g069-c1 correctly refutes shell-command injection and
leading-option injection using quoting and the leading-alphanumeric grammar. Neither defense refutes trailing action
suffix interpretation inside a permitted argv word; no contradiction and no reason to withdraw l1. Suggested fix: make
package selections unambiguously install-only while preserving valid literal package names, or reject ambiguous
removal-shaped selectors with a clear message. Suggested bucket: other, preserving the fresh-builder/reproducible-state
consequence; no existing user work, running host, or credential-loss route is established. Confidence: confirmed static
semantics/route; exact image outcome conditional as above. Temporal qualification: validation is introduced at
68790603037bb51ccd8655e4a948c7ffb747760a, acquires this behavior only when its builder consumer ships at
744ea4122d033ef616454fe592ecb2173593736c, and the prior source checks found no validator or consumer repair through
pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b. proposed_drop: false. possible_cover: unknown; forbidden coverage
sources not searched. Synthesis resolves the temporary l1 recheck lead. No runtime tests, package-manager execution,
network experiments or cloud calls; only primary documentation/source browsing and pinned repository source reads.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g063/l1`; supporting records
`g069/g069-s1`. The retained private log preserves earlier wording and its corrections. This record does not claim
runtime reproduction.
