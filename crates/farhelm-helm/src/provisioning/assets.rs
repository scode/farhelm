//! The release asset inventory: the exact archive and binary names a
//! GitHub release publishes, and every fact derived from them.
//!
//! This is the AUTHORITATIVE RUST inventory (plan §1), not a single source
//! every consumer literally shares. Every Rust payload source reads
//! directly from it: both
//! [`DirectoryPayloads`](super::payloads::DirectoryPayloads) and the
//! verified download source
//! [`ReleasePayloadSource`](super::release_payloads::ReleasePayloadSource).
//! `install.sh`'s asset table and the `SHA256SUMS`-writing release workflow
//! are NOT Rust and cannot import `RELEASE_ARCHIVES` or `sums_members()` —
//! they keep their own representations (a delimited shell table, a YAML
//! block) — so this module's test module reads each of those files back and
//! compares. The `sign-sums.yml` parity test is here already; `install.sh`'s
//! arrives with the script in Step 6. Update this module first when a target
//! or package changes; the parity tests are what catch the other two falling
//! behind.

use super::plan::PayloadArch;

/// One `.tar.gz` release archive: a cargo-dist package built for one target
/// triple, holding one bare binary (`member`) nested under
/// `<package>-<target>/`. That layout is dist's own, confirmed against dist
/// 0.32.0's output, and is why callers locate `member` by basename rather
/// than by an exact in-archive path.
///
/// `member` is the only EXECUTABLE in the archive but not the only file: a
/// copy of `LICENSE` sits beside it (`include` in `dist-workspace.toml`).
/// Anything that unpacks one of these must therefore select the member it
/// wants rather than assume a single entry — and the `sign-sums` job asserts
/// the file list is exactly those two, so a third file cannot appear
/// unnoticed.
pub struct ReleaseArchive {
    pub package: &'static str,
    pub target: &'static str,
    pub member: &'static str,
}

/// Every archive a release publishes (D1, D4, D6): the CLI/helm/supervisor
/// binary for the three shipped targets, plus the macOS desktop shell.
/// `farhelm_archive_for` narrows this to the two Linux entries provisioning
/// actually installs on a host; the macOS entries exist only for
/// `install.sh` and never travel over SSH.
pub const RELEASE_ARCHIVES: [ReleaseArchive; 4] = [
    ReleaseArchive {
        package: "farhelm",
        target: "x86_64-unknown-linux-musl",
        member: "farhelm",
    },
    ReleaseArchive {
        package: "farhelm",
        target: "aarch64-unknown-linux-musl",
        member: "farhelm",
    },
    ReleaseArchive {
        package: "farhelm",
        target: "aarch64-apple-darwin",
        member: "farhelm",
    },
    ReleaseArchive {
        package: "farhelm-desktop",
        target: "aarch64-apple-darwin",
        member: "farhelm-desktop",
    },
];

/// The published archive filename for one [`ReleaseArchive`] — dist's
/// `unix-archive = ".tar.gz"` (D14) made concrete, not a coincidental
/// convention this module invented on its own.
pub fn archive_name(archive: &ReleaseArchive) -> String {
    format!("{}-{}.tar.gz", archive.package, archive.target)
}

/// The bare (unarchived) static tmux build published for one Linux payload
/// architecture (D5) — the one payload kind released without a `.tar.gz`
/// wrapper, so it needs no [`ReleaseArchive`] entry of its own.
pub fn tmux_name(arch: PayloadArch) -> &'static str {
    match arch {
        PayloadArch::X86_64 => "tmux-x86_64-unknown-linux-musl",
        PayloadArch::Aarch64 => "tmux-aarch64-unknown-linux-musl",
    }
}

/// The Linux host payload archive for `arch`: provisioning only ever
/// installs the `farhelm` package (never `farhelm-desktop`) onto a remote
/// host, and only onto a musl Linux target (D4) — so despite
/// [`RELEASE_ARCHIVES`] carrying entries `PayloadArch` cannot name (macOS,
/// the desktop shell), this lookup is total over every `PayloadArch`.
pub fn farhelm_archive_for(arch: PayloadArch) -> &'static ReleaseArchive {
    let target = match arch {
        PayloadArch::X86_64 => "x86_64-unknown-linux-musl",
        PayloadArch::Aarch64 => "aarch64-unknown-linux-musl",
    };
    RELEASE_ARCHIVES
        .iter()
        .find(|archive| archive.package == "farhelm" && archive.target == target)
        .expect("RELEASE_ARCHIVES carries a farhelm archive for every PayloadArch target")
}

/// The exact six names `SHA256SUMS` lists, sorted — every [`RELEASE_ARCHIVES`]
/// entry plus both tmux builds. The `sign-sums` release job hashes exactly
/// this list, in this order, out of its own hardcoded copy; the parity test
/// below reads that copy back out of the workflow and compares, which is what
/// keeps the checksum file and the helm's download expectations from drifting
/// apart.
///
/// Still called from nowhere but tests, and deliberately so: the download
/// source looks each asset up by name in whatever `SHA256SUMS` it fetched
/// rather than demanding this exact file list, because a release that grows a
/// seventh asset must not make every older helm refuse it. The consumers this
/// exists for are the `sign-sums` job and `install.sh`'s asset table
/// (Step 6), neither of which is Rust.
#[allow(dead_code)]
pub fn sums_members() -> Vec<String> {
    let mut members: Vec<String> = RELEASE_ARCHIVES.iter().map(archive_name).collect();
    members.push(tmux_name(PayloadArch::X86_64).to_string());
    members.push(tmux_name(PayloadArch::Aarch64).to_string());
    members.sort();
    members
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec: the published archive filename is `{package}-{target}.tar.gz`
    /// for every entry — the literal contract `install.sh`'s asset table
    /// and the directory/download payload sources rely on.
    #[test]
    fn archive_name_is_package_dash_target_dot_tar_gz() {
        assert_eq!(
            archive_name(&RELEASE_ARCHIVES[0]),
            "farhelm-x86_64-unknown-linux-musl.tar.gz"
        );
        assert_eq!(
            archive_name(&RELEASE_ARCHIVES[3]),
            "farhelm-desktop-aarch64-apple-darwin.tar.gz"
        );
    }

    /// Spec: `tmux_name` names the two published Linux tmux builds and
    /// nothing else — there is no macOS or Windows tmux payload to confuse
    /// this with.
    #[test]
    fn tmux_name_covers_both_linux_architectures() {
        assert_eq!(
            tmux_name(PayloadArch::X86_64),
            "tmux-x86_64-unknown-linux-musl"
        );
        assert_eq!(
            tmux_name(PayloadArch::Aarch64),
            "tmux-aarch64-unknown-linux-musl"
        );
    }

    /// Spec: both provisioning architectures resolve to a musl Linux
    /// `farhelm` archive (D4) — never the macOS or desktop-shell entries
    /// [`RELEASE_ARCHIVES`] also carries.
    #[test]
    fn farhelm_archive_for_maps_both_arches_to_musl_archives() {
        for (arch, target) in [
            (PayloadArch::X86_64, "x86_64-unknown-linux-musl"),
            (PayloadArch::Aarch64, "aarch64-unknown-linux-musl"),
        ] {
            let archive = farhelm_archive_for(arch);
            assert_eq!(archive.package, "farhelm");
            assert_eq!(archive.target, target);
            assert_eq!(archive.member, "farhelm");
        }
    }

    /// Spec: `sums_members` is exactly the six published names — four
    /// archives plus two tmux builds — sorted, which is the order
    /// `SHA256SUMS` itself must list them in (plan §1).
    #[test]
    fn sums_members_is_exactly_the_six_sorted_names() {
        // Already lexically sorted (F25, review round 1): the literal
        // itself expresses the required order, so nothing here mutates it.
        let expected = vec![
            "farhelm-aarch64-apple-darwin.tar.gz".to_string(),
            "farhelm-aarch64-unknown-linux-musl.tar.gz".to_string(),
            "farhelm-desktop-aarch64-apple-darwin.tar.gz".to_string(),
            "farhelm-x86_64-unknown-linux-musl.tar.gz".to_string(),
            "tmux-aarch64-unknown-linux-musl".to_string(),
            "tmux-x86_64-unknown-linux-musl".to_string(),
        ];
        assert_eq!(sums_members(), expected);
    }

    /// Spec: the asset list hardcoded in the `sign-sums` release job equals
    /// [`sums_members`], name for name and in the same order.
    ///
    /// Why this matters more than it looks: that job is what turns a set of
    /// uploaded files into the signed `SHA256SUMS` every helm verifies
    /// against. If the workflow's list falls behind this module — a target
    /// added here, a name changed — CI still produces a perfectly valid
    /// signature over a SHORTER list, and the failure surfaces as a helm
    /// refusing to provision because an asset it wants is "not in
    /// SHA256SUMS". YAML cannot import Rust, so the two lists are separate by
    /// necessity; this test is what makes the duplication safe.
    #[test]
    fn sign_sums_workflow_lists_exactly_sums_members() {
        const WORKFLOW: &str = include_str!("../../../../.github/workflows/sign-sums.yml");
        const BEGIN: &str = "# BEGIN SUMS MEMBERS";
        const END: &str = "# END SUMS MEMBERS";

        let block = WORKFLOW
            .split_once(BEGIN)
            .and_then(|(_, rest)| rest.split_once(END))
            .map(|(block, _)| block)
            .unwrap_or_else(|| {
                panic!("sign-sums.yml has no {BEGIN}/{END} block; did the markers get renamed?")
            });

        // The block is `SUMS_MEMBERS: |` followed by one indented name per
        // line. Dropping the key line and trimming leaves exactly the names,
        // which keeps this parser indifferent to how the YAML is indented.
        let listed: Vec<String> = block
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with("SUMS_MEMBERS:"))
            .map(str::to_owned)
            .collect();

        assert_eq!(
            listed,
            sums_members(),
            "the asset list in .github/workflows/sign-sums.yml has drifted from sums_members()"
        );
    }

    /// Strictly parses the exact `ASSET_TABLE='...'` shell wrapper
    /// `scripts/install.sh` carries between its `# BEGIN/END ASSET TABLE`
    /// markers: a blank line, then `ASSET_TABLE='` (however indented — see
    /// below), then one `TARGET|ARCHIVE|BINARY` row per line — each with
    /// exactly three nonempty, UNINDENTED fields — then a closing `'`
    /// (again, however indented) alone on its own line. Returns `Err`
    /// describing what did not match, rather than panicking, so both the
    /// real parity test below and the malformed-input tests can inspect
    /// the failure directly instead of relying on `#[should_panic]`'s much
    /// weaker "some panic happened somewhere" guarantee.
    ///
    /// The strictness is not pedantry: the shell reads this table with
    /// `IFS='|' read -r target archive binary`, so a field this parser
    /// would casually trim or a row it would silently drop means the shell
    /// and this test stop agreeing on what the table says. A field with
    /// leading whitespace still compares unequal to `uname`'s bare output
    /// and permanently fails to match at runtime; a row with a missing or
    /// extra `|` gets fields silently mis-assigned by `read` rather than
    /// rejected. Order and multiplicity are preserved (`Vec`, not a
    /// `HashSet`/`BTreeSet`): the shell reads this table top to bottom, so
    /// a duplicated or reordered row is a real behavioral difference from
    /// [`RELEASE_ARCHIVES`], not a cosmetic one a set-based comparison
    /// would quietly absorb.
    ///
    /// The `ASSET_TABLE='` and closing `'` wrapper lines ARE trimmed before
    /// comparison, unlike the row lines: they are ordinary shell statements
    /// that pick up whatever indentation the surrounding code block is at
    /// (install.sh's whole body lives inside a `{ ... }` group, so this is
    /// indented two spaces in practice), and indentation there changes
    /// nothing about how the shell reads them. The ROW lines are different:
    /// they are literal bytes inside a multi-line single-quoted string, so
    /// indenting one would inject that whitespace into `$row_target` and
    /// break the shell's own `[ "$row_target" = "$TARGET" ]` comparison at
    /// runtime — which is exactly the leading-whitespace case this parser
    /// must keep rejecting.
    fn parse_asset_table_rows(block: &str) -> Result<Vec<(String, String, String)>, String> {
        let lines: Vec<&str> = block.lines().collect();
        if lines.len() < 3 {
            return Err(format!(
                "ASSET TABLE block has only {} line(s); expected at least an opening blank \
                 line, `ASSET_TABLE='`, and a closing `'`",
                lines.len()
            ));
        }
        if !lines[0].is_empty() {
            return Err(format!(
                "ASSET TABLE block's first line must be blank, got {:?}",
                lines[0]
            ));
        }
        if lines[1].trim() != "ASSET_TABLE='" {
            return Err(format!(
                "ASSET TABLE block's second line must be (possibly indented) `ASSET_TABLE='`, got {:?}",
                lines[1]
            ));
        }
        let last = lines[lines.len() - 1];
        if last.trim() != "'" {
            return Err(format!(
                "ASSET TABLE block's last line must be (possibly indented) `'`, got {:?}",
                last
            ));
        }

        let mut rows = Vec::new();
        for line in &lines[2..lines.len() - 1] {
            let fields: Vec<&str> = line.split('|').collect();
            if fields.len() != 3 {
                return Err(format!(
                    "ASSET TABLE row {line:?} has {} `|`-delimited field(s), expected exactly 3",
                    fields.len()
                ));
            }
            for field in &fields {
                if field.is_empty() {
                    return Err(format!("ASSET TABLE row {line:?} has an empty field"));
                }
                if field.trim() != *field {
                    return Err(format!(
                        "ASSET TABLE row {line:?} has leading or trailing whitespace in a field"
                    ));
                }
            }
            rows.push((
                fields[0].to_string(),
                fields[1].to_string(),
                fields[2].to_string(),
            ));
        }
        Ok(rows)
    }

    /// Spec: `scripts/install.sh`'s asset table is EXACTLY the same
    /// sequence of (target, archive, binary) rows as [`RELEASE_ARCHIVES`]
    /// — same rows, same order, same count, not merely "the same set".
    ///
    /// The script is not Rust and cannot `use` this module directly (plan
    /// §1), so this equality check — riding on `parse_asset_table_rows`'s
    /// strict validation of the table's shell-level shape — is the only
    /// thing that keeps the two from drifting apart, the way
    /// `sign_sums_workflow_lists_exactly_sums_members` keeps the release
    /// workflow honest. The `parser_*` tests below exercise malformed and
    /// duplicated input this equality check alone would not distinguish
    /// from an already-passing table.
    #[test]
    fn install_sh_asset_table_matches_release_archives_exactly() {
        const SCRIPT: &str = include_str!("../../../../scripts/install.sh");
        const BEGIN: &str = "# BEGIN ASSET TABLE";
        const END: &str = "# END ASSET TABLE";

        let block = SCRIPT
            .split_once(BEGIN)
            .and_then(|(_, rest)| rest.split_once(END))
            .map(|(block, _)| block)
            .unwrap_or_else(|| {
                panic!("install.sh has no {BEGIN}/{END} block; did the markers get renamed?")
            });

        let script_rows = parse_asset_table_rows(block).unwrap_or_else(|error| {
            panic!("install.sh's ASSET TABLE block does not parse: {error}")
        });

        let expected_rows: Vec<(String, String, String)> = RELEASE_ARCHIVES
            .iter()
            .map(|a| (a.target.to_string(), archive_name(a), a.member.to_string()))
            .collect();

        assert_eq!(
            script_rows, expected_rows,
            "scripts/install.sh's ASSET TABLE block has drifted from RELEASE_ARCHIVES: install.sh reads this \
             table top to bottom and installs one row per platform match, so order and exact row multiplicity \
             both matter, not just which rows are present"
        );
    }

    /// Spec: a row whose TARGET field carries leading indentation is
    /// rejected rather than silently accepted — the shell's own `IFS='|'
    /// read` would fold that whitespace into the compared value, so a
    /// target like `" aarch64-apple-darwin"` never matches `uname`'s bare
    /// output and the row becomes permanently unreachable at runtime.
    #[test]
    fn parser_rejects_leading_whitespace_in_a_field() {
        let block = "\nASSET_TABLE='\n aarch64-apple-darwin|farhelm-aarch64-apple-darwin.tar.gz|farhelm\n'\n";
        assert!(parse_asset_table_rows(block).is_err());
    }

    /// Spec: a row with too few `|`-delimited fields is rejected rather
    /// than silently accepted with a blank trailing field — `read
    /// row_target row_archive row_binary` would otherwise leave
    /// `row_binary` empty and the row would try to install a binary named
    /// "".
    #[test]
    fn parser_rejects_a_row_with_a_missing_field() {
        let block =
            "\nASSET_TABLE='\naarch64-apple-darwin|farhelm-aarch64-apple-darwin.tar.gz\n'\n";
        assert!(parse_asset_table_rows(block).is_err());
    }

    /// Spec: a row with an extra `|`-delimited field is rejected rather
    /// than silently absorbed into the last named variable — `IFS='|'
    /// read -r a b c` assigns every field past the third onto `c`, which
    /// would smuggle a fourth column into the binary name unnoticed.
    #[test]
    fn parser_rejects_a_row_with_an_extra_field() {
        let block = "\nASSET_TABLE='\naarch64-apple-darwin|farhelm-aarch64-apple-darwin.tar.gz|farhelm|extra\n'\n";
        assert!(parse_asset_table_rows(block).is_err());
    }

    /// Spec: a row with an empty field (e.g. two adjacent `|`s) is
    /// rejected rather than producing an empty target, archive, or binary
    /// name.
    #[test]
    fn parser_rejects_a_row_with_an_empty_field() {
        let block = "\nASSET_TABLE='\naarch64-apple-darwin||farhelm\n'\n";
        assert!(parse_asset_table_rows(block).is_err());
    }

    /// Spec: the parser preserves a duplicated row rather than quietly
    /// deduplicating it — a `HashSet`/`BTreeSet`-based parser would fold
    /// two identical rows into one and hide a table that downloads and
    /// verifies the same archive twice for no benefit. Deduplication (or
    /// any other multiplicity mismatch against [`RELEASE_ARCHIVES`]) is
    /// what `install_sh_asset_table_matches_release_archives_exactly`'s
    /// `Vec`-based equality check above actually catches; this test proves
    /// the parser gives it real duplicate rows to catch in the first
    /// place.
    #[test]
    fn parser_preserves_duplicate_rows_rather_than_deduplicating_them() {
        let block = "\nASSET_TABLE='\n\
             x86_64-unknown-linux-musl|farhelm-x86_64-unknown-linux-musl.tar.gz|farhelm\n\
             x86_64-unknown-linux-musl|farhelm-x86_64-unknown-linux-musl.tar.gz|farhelm\n\
             '\n";
        let rows = parse_asset_table_rows(block).expect("well-formed rows should parse");
        assert_eq!(
            rows.len(),
            2,
            "the parser must preserve multiplicity, not deduplicate"
        );
    }

    /// Spec: the generic dist package that builds `farhelm-desktop` declares
    /// the same version as this workspace.
    ///
    /// cargo-dist announces a tag by matching package versions, and that
    /// package's version is a hand-written literal rather than something
    /// inherited from Cargo. Let the two diverge and a `vX.Y.Z` tag announces
    /// only `farhelm`: the desktop archive disappears from the release with
    /// no error anywhere — dist reports it as an informational note about
    /// "multiple version numbers" — and the first sign of trouble is a Mac
    /// user's `install.sh` failing to find its asset. A version bump is
    /// exactly when this is easiest to forget, so the check lives in the
    /// suite that runs on every change.
    #[test]
    fn desktop_dist_package_version_matches_the_workspace() {
        const MANIFEST: &str = include_str!("../../../../packaging/farhelm-desktop/dist.toml");

        let versions: Vec<&str> = MANIFEST
            .lines()
            .map(str::trim)
            .filter_map(|line| line.strip_prefix("version = \""))
            .filter_map(|rest| rest.strip_suffix('"'))
            .collect();

        assert_eq!(
            versions.len(),
            1,
            "expected exactly one version key in packaging/farhelm-desktop/dist.toml, found {versions:?}"
        );
        assert_eq!(
            versions[0],
            env!("CARGO_PKG_VERSION"),
            "packaging/farhelm-desktop/dist.toml's version must track [workspace.package].version, \
             or a tagged release silently drops the desktop archive"
        );
    }
}
