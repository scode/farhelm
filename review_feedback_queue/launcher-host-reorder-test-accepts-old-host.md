# The Host menu test leaves rendered row order and a distinct Enter target unasserted

Reviewed commit: 30441792a456f98227643624e65ae76620a5fb64

## TLDR

The Host menu reorder test can pass without demonstrating that Enter selected the visible host.

## Details

**other.** Source inspection.

crates/gui/farhelm-gui/src/launcher/view_tests.rs:2721–2760 at 30441792a456f98227643624e65ae76620a5fb64 and 2952–2991 at
1a53d9d35889d7dde96e31320cc1681dd013737d

The test could miss a Host menu that ignores the helm's updated order or that re-chooses the old launch destination
instead of applying a distinct highlight. At 1a53d9d35889d7dde96e31320cc1681dd013737d, view_tests.rs:2975–2984 supplies
order [2, 1] and checks selected states by element ID. The helper at 2747–2752 only finds the ID and reads its selected
flag. It does not compare node positions or sequence. The shipped path does honor the supplied order:
test_support.rs:470–479 builds the metadata, crates/farhelm-helm-client/src/collections.rs:323–327,335–349 installs and
traverses it, and launcher/view/host_picker.rs:120–134,556–611 keeps the eligible existing-host order and appends the
popover rows in that order. View_tests.rs:2956–2959,2985–2991 starts and ends with host 1, checking the explicit-pick
flag. Re-picking host 1 also sets that flag (host_picker.rs:311–321). The test does catch a positional regression that
selects host 2 when the reorder takes effect. The actual Enter handler uses the stored active ID and resolves current
eligibility (host_picker.rs:181–190,297–308); the review found a test gap on this route. Assert the rendered row order
and make the highlighted host differ from the draft host. SPEC_impl.md:9389–9396 at the pinned tip describes open-list
Enter and distinguishes browsing from a pick. The reviewer suggested other. Confidence confirmed by source. established.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-form-checks/LF-1`.
