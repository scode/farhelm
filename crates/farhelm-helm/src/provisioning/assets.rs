//! The release asset inventory: the exact archive and binary names a
//! GitHub release publishes, and every fact derived from them.
//!
//! This is the AUTHORITATIVE RUST inventory (plan §1), not a single source
//! every consumer literally shares. Every Rust payload source reads
//! directly from it: [`DirectoryPayloads`](super::payloads::DirectoryPayloads)
//! today, Step 3b's download source next. `install.sh`'s asset table and
//! the `SHA256SUMS`-writing release workflow are NOT Rust and cannot import
//! `RELEASE_ARCHIVES` or `sums_members()` — they keep their own
//! representations (a delimited shell table, workflow config) — but Step 6
//! adds a parity test that checks each of those against this module, so the
//! three descriptions are validated against one another rather than left to
//! drift silently apart. Update this module first when a target or package
//! changes; the parity test is what catches the other two falling behind.

use super::plan::PayloadArch;

/// One `.tar.gz` release archive: a cargo-dist package built for one target
/// triple, holding exactly one bare binary (`member`) nested under
/// `<package>-<target>/` — dist's own archive layout (VALIDATE in plan
/// Step 5), which is why callers locate `member` by basename rather than by
/// an exact in-archive path.
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
/// entry plus both tmux builds. `sign-sums` (Step 5) writes this same list;
/// keeping the computation here rather than duplicating it in that job's
/// script is what keeps the checksum file and the helm's own download
/// expectations (Step 3b) from drifting apart.
///
/// Unused outside this module's own tests until Step 3b's download source
/// reads `SHA256SUMS` and Step 6's `install.sh` test compares against it —
/// landed now, with plan §1's exact signature, so those steps add no new
/// public surface to this module.
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
}
