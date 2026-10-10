# An incomplete `/latest` response can select the wrong release

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A truncated latest-version response could install an unintended older release.

## Details

F115 — **possible** — `scripts/install.sh:862` — An incomplete `/latest` response can select the wrong release

The unpinned installer discards curl's transfer failure after receiving the latest-version response. A truncated HTTP
200 body ending at a valid version prefix can pass validation and select that prefix's release. If a compatible release
exists, later checks validate the wrong selection and installation can succeed; a downgrade additionally requires it to
be older than the installed version. Require successful transfer before accepting the body, retaining HTTP status
separately for diagnostics. Pinned installations bypass this lookup.

## Evidence and triage context

- scripts/install.sh:217-225: curl_get invokes curl directly and preserves its exit status; it does not buffer the
  response until successful completion.
- scripts/install.sh:851-882: an unpinned installation requests /latest at line 862 with `|| true`; lines 863-869
  separate the appended HTTP status and response body, and lines 875-877 reject only non-200 HTTP status. A partial body
  `v1.2.3` followed by curl's appended newline and `200` remains an accepted candidate.
- scripts/install.sh:88 and scripts/install.sh:119-128: the version grammar accepts v1.2.3 without requiring a trailing
  newline. Lines 894-907 normalize that candidate and reject prereleases, but cannot distinguish a valid prefix from a
  complete response.
- scripts/install.sh:909-936: VERSION_NUM and BASE_URL derive from the accepted prefix; SHA256SUMS is then fetched from
  that release's directory.
- scripts/install.sh:1005-1037 and scripts/install.sh:1081-1106: archive checksums, the CLI's reported version, icon
  presence, and desktop compatibility marker all validate the selected release. They do not recover the intended latest
  version.
- scripts/install.sh:1236-1264: an existing installation's previous version is read for retention, without a
  newer-version comparison; the selected desktop executable and Installed record are published. Lines 1435-1439 report
  success.
- scripts/install.sh:1471: the script directly invokes main. The affected path is reachable during ordinary unpinned
  manual installation.
- crates/farhelm-ui/src/desktop/updater.rs:993-1015: the desktop updater explicitly pins FARHELM_VERSION when invoking
  the installer, so that caller bypasses this installer /latest lookup.
- website/src/content/docs/docs/get-started/install.md:18 documents an unpinned installer invocation;
  scripts/install.sh:1471 calls main, and scripts/install.sh:852–856 selects the latest lookup when FARHELM_VERSION is
  unset.
- scripts/install.sh:217–225 invokes curl directly and preserves its exit status. scripts/install.sh:862 discards that
  status with || true while retaining partial stdout and curl's HTTP-status write-out.
- scripts/install.sh:863–881 extracts the body and accepts HTTP 200 without requiring transfer success or a terminal
  body newline. A partial body such as v1.2.1 from an intended v1.2.12 therefore reaches normalization.
- scripts/install.sh:88 and scripts/install.sh:119–127 accept that shortened stable tag. scripts/install.sh:894–912
  derives VERSION_NUM and BASE_URL from it.
- scripts/install.sh:935–952 fetches checksums under the selected tag; scripts/install.sh:1016–1037 downloads and checks
  that tag's archives. scripts/install.sh:1081–1085 compares the binary version against the same mistakenly selected
  VERSION_NUM, so these checks do not establish the intended latest version.
- scripts/install.sh:1090–1106 refuses incompatible old releases, limiting the trigger to a shortened tag whose archives
  satisfy the app-resource and side-by-side-layout requirements.
- scripts/install.sh:1236–1264 reads the previously installed version but performs no newer-version comparison before
  replacing the desktop executable and Installed record. scripts/install.sh:1435–1439 then reports success for
  VERSION_NUM.
- crates/farhelm-ui/src/desktop/updater.rs:993–1015 explicitly pins its installer subprocess to the chosen version,
  bypassing this lookup.

Retained confidence: **possible**. Suggested bucket: **high**.

Possible cover:

- {"basis": "SPEC.md:280-285, Installation and updates", "comparison": "The default must select the latest stable
  release. Support for interrupted installation leaving an old or new working app does not accept interpreting an
  incomplete lookup as a different release."}
- {"basis": "SPEC.md:321-328, Installation and updates", "comparison": "Trusting the origin over TLS addresses
  authenticity and signature verification. It does not accept ignoring a detected transport failure or installing a
  truncated version name."}
- {"basis": "SPEC_impl.md:3481-3487, Verification chain (D3)", "comparison": "The origin contract defines /latest as the
  latest stable tag. It provides no exception for partial transfers."}
- {"basis": "review_feedback_queue/installer-startup-prune.md:14-27", "comparison": "This concerns an interrupted bundle
  update followed by concurrent supervisor startup and pruning, causing loss of a needed executable. Its trigger and
  mechanism differ from accepting an incomplete release lookup."}
- {"basis": "review_feedback_queue/installer-directory-target.md:14-25", "comparison": "This concerns a directory
  occupying an executable destination and mv reporting success. It does not cover release selection from partial network
  data."}
- {"basis": "review_feedback_queue/provisioning-update-replaces-binary-before-setup-guard.md:7-11", "comparison": "This
  concerns remote provisioning replacing a setup-managed binary before checking ownership. A possible downgrade is
  shared terminology, but the trigger, installation path, and required correction differ."}
- {"basis": "TRIAGE_OUTCOMES.md:5759-5771, partial-release-download-left-behind.md", "comparison": "That decision
  concerns leftover partial asset files in the helm's cache, explicitly never mistaken for verified downloads. Here
  partial metadata becomes an accepted release selection."}
- {"basis": "review_feedback_queue/FILTER.md:24-42, Rare, self-correcting glitches and imprecise diagnostics",
  "comparison": "Although the trigger is narrow, the consequence is a durable installation of the wrong release with
  success reported. It is not a safe failed operation or transient diagnostic."}
- {"basis": "TRIAGE_OUTCOMES.md:5759, partial-release-download-left-behind.md", "comparison": "Not coverage: this
  concerns failed helm payload downloads leaving partial files that are never mistaken for verified downloads. The
  finding concerns standalone-installer metadata truncation selecting and successfully installing a different release."}
- {"basis": "TRIAGE_OUTCOMES.md:5475, update-silently-downgrades-newer-hosts.md", "comparison": "Not coverage: this
  concerns an older helm replacing a newer remote host's binary. It does not cover failed latest-version transfers in
  the standalone Mac installer."}
- {"basis": "review_feedback_queue/installer-directory-target.md, Details;
  review_feedback_queue/installer-startup-prune.md, Details", "comparison": "Neither covers this mechanism. Their
  triggers are malformed destination directories and interrupted-update startup/pruning races, respectively; neither
  addresses partial version metadata selecting another release."}
- {"basis": "review_feedback_queue/provisioning-update-replaces-binary-before-setup-guard.md, Details", "comparison":
  "Not coverage: its trigger is setup taking ownership after remote-update planning, with binary replacement preceding
  the ownership guard. Both trigger and installation scope differ."}
- {"basis": "SPEC.md:280–285 and SPEC.md:321–328; SPEC_impl.md:3481–3487", "comparison": "The contracts specify
  latest-stable selection and intentionally trust the origin over TLS. They do not accept interpreting a detected
  incomplete transfer as a complete answer. Intentional trust of the origin does not cover this failure."}
- {"basis": "review_feedback_queue/FILTER.md:24–42, Rare, self-correcting glitches and imprecise diagnostics",
  "comparison": "Not applicable: the conditional consequence is a durably installed wrong version with success reported,
  rather than a safe failed operation or transient diagnostic."}
- {"basis": "TODO.md:33–43, Planned; BUGS.md:8, :53, :80", "comparison": "Planned covers supervisor request dispatch.
  The known bugs concern tmux crashes, idle-terminal memory growth, and SSH-helper hangs. None matches this trigger,
  consequence, and scope."}

Caveats:

- Verified by bounded source inspection at bd8d5d76439f8f36bbc2637b44d7ba4026b65675; no runtime tests were run.
- Requires an incomplete response ending at a valid version prefix whose compatible release is available. The example
  versions are illustrative, not verified published releases.
- Downgrading additionally requires the selected prefix version to be older than the existing installation.
- No concrete credential exposure, exploitable vulnerability reintroduction, or user-data loss was established; those
  consequences should not be asserted.
- The desktop updater's pinned installer invocation is outside this specific defect.
- No runtime reproduction or actual published, installer-compatible tag pair was established.
- A missing shortened release or incompatible archive causes refusal rather than installation.
- Pinned installations, including the desktop updater's installer invocation, bypass the affected lookup.
- This establishes a possible unintended downgrade, not demonstrated credential loss, user-work loss, or a security
  exploit.
- Verification was read-only against the requested frozen commit; no runtime tests, checkout writes, VCS mutations, or
  agents were used.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_10_sec:p2:F1`,
`cli_installation_10_cor:p2:F1`.

- `cli_installation_10_sec:p2:F1`: confidence as filed: definite; suggested bucket as filed: high.
- `cli_installation_10_cor:p2:F1`: confidence as filed: possible; suggested bucket as filed: other.
