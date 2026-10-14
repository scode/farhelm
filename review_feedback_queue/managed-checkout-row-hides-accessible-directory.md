# Retain the working folder in full rows’ accessible text

Reviewed commit: 148b0c98e4ec3545a8465883f22498a005ff247d

## TLDR

Screen-reader users lose the working directory when browsing managed-checkout sessions in the default, noncompact list.

## Details

Suggested bucket: `other`. Confidence: confirmed by static markup and tooltip tracing; no screen-reader runtime
reproduction. Source: dedicated `gpt-6.1-sol` high adversarial review. Proposed drop: none.

At crates/farhelm-ui/src/list/row.rs:1675–1676, the managed-checkout mark replaces the directory text. Its output in
crates/farhelm-ui/src/icons.rs:95–112 includes the directory only in data-tooltip; readable descendants contain just
“managed checkout” and the repository. The tooltip element is explicitly aria-hidden (assets/tooltip.js:213–218), and
the hidden directory copy in row.rs:1701–1702 remains conditional on compact mode. Before this commit, noncompact rows
contained directory text (first parent, row.rs:1673–1674). Consequently, users browsing the accessible list cannot
distinguish checkout locations or borrower subdirectories without opening each session. Preserve a hidden directory copy
for managed noncompact rows, or expose it through an accessible description. Scope: list metadata; the opened session
still exposes its directory.

Possible cover: SPEC.md:640–645 and SPEC_impl.md:610–615 intentionally replace the visible directory with the
repository, but do not explicitly accept removing the accessible directory. SPEC_impl.md:670–673 requires needed hover
information to have an accessible counterpart. TRIAGE_OUTCOMES.md heading titles-raw-in-confirm-prompts.md excludes
accessibility-label escaping from that particular fix, not directory accessibility generally. No matching queue, Planned
item, BUGS entry, or filter found.

Status at frozen endpoint: Persists at pinned main 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: row.rs:1721–1722 still
replaces the directory; its hidden copy at 1747–1748 remains compact-only; icons.rs:107–124 still exposes the directory
only through data-tooltip. No fixing commit identified.

Review coverage: full commit diff read: true. No commit diff hunks unread. Unchanged surrounding modules were inspected
selectively. The 13,089-line triage ledger was searched throughout and relevant sections read, not read end-to-end.
Unrelated queue files were represented by their index entries rather than all being opened. Initially truncated
aggregate output was reopened for the relied-upon diff and row ranges.

Runtime evidence: None; static review only, as requested. No browser, geometry, or screen-reader reproduction. Applied
the supplied test-authoring checklist to changed tests and fixture interactions. Created, modified, and deleted nothing
in the checkout.

## Additional affected scope

Reviewed commit: 148b0c98e4ec3545a8465883f22498a005ff247d

## TLDR

Screen-reader users lose the working directory from full session rows and cannot read the new mark’s explanation of when
the checkout moves to trash.

## Details

Suggested bucket: `other`. Confidence: confirmed by static markup and tooltip implementation; no screen-reader runtime
reproduction. Source: dedicated `gpt-6-astra` high adversarial review. Proposed drop: none.

At crates/farhelm-ui/src/list/row.rs:1675–1676, the managed-checkout mark replaces the directory text. The replacement
at crates/farhelm-ui/src/icons.rs:106–112 exposes only “managed checkout” and the repository to assistive technology;
the actual checkout path, differing session directory, and lifetime explanation exist only in data-tooltip.
assets/tooltip.js:213–218 explicitly hides that tooltip from the accessibility tree. The hidden directory fallback at
row.rs:1701–1702 applies only to compact rows. Thus ordinary noncompact managed rows no longer identify their directory
accessibly, and neither mark variant exposes the lifetime explanation. Opening each session still permits reading its
directory through the header copy control, but does not repair the list regression. Preserve the relevant description in
hidden text or an accessible description. The new browser helper checks the abbreviated hidden label and tooltip
attribute, so it does not detect this omission.

Possible cover: SPEC_impl.md:610–615 specifies the visible repository and hidden identifying word, but does not
explicitly accept losing accessible location/lifetime information. Its tooltip contract at 669–673 requires necessary
hover information to have an accessible counterpart. No matching Planned item, BUGS entry, queue index item, or ledger
heading found.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: icons.rs:102–125 retains the same
implementation; row.rs:1721–1748 still supplies the hidden directory only in compact mode. No fixing commit identified.

Review coverage: full commit diff read: true. No commit diff hunks unread. Unchanged portions of large modules and most
ledger entry bodies were not read exhaustively. Individual original queue finding files were not opened because the
original index identified no matching accessibility item. No Sol reports or working-copy queue additions were read.
Browser geometry and actual assistive-technology behavior were not exercised.

Runtime evidence: None, as requested by default. Static review only; no checkout files created, modified, or deleted, no
helper agents, and no fetch or range extension.
