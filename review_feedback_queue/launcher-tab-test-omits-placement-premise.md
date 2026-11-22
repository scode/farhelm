# The all-placements tab-order check lost its premise that both environment offers are available

Reviewed commit: 30441792a456f98227643624e65ae76620a5fb64

## TLDR

The launcher test for all placements can pass when its fixture omits an environment offer.

## Details

**other.** Source inspection.

crates/gui/farhelm-gui/src/launcher/view_tests.rs:479–533 at 30441792a456f98227643624e65ae76620a5fb64, still at 598–653
at 1a53d9d35889d7dde96e31320cc1681dd013737d

The launcher test can pass with New sandbox or New container missing from its fixture, losing coverage of the intended
keyboard path when all destinations are offered. At 82c5c23129a913833d0a343026870ccbbc9579e5, view_tests.rs:511–534
actually reached both provisioning controls by Tab. At 30441792a456f98227643624e65ae76620a5fb64, view_tests.rs:497–529
supplies account data and checks the closed form's tab order, but no longer checks that the Host menu offers either
environment. At 1a53d9d35889d7dde96e31320cc1681dd013737d, view_tests.rs:616–650 still replies only to requests that
arrive and asserts the recent setup and closed-form focus stops. For example, if the account read is absent this test
has no assertion that will detect that premise failing, while the sandbox offer requires the API key from that read
(launcher/view/placement.rs:63–74,129–161 and crates/gui/farhelm-gui/src/environment_ui.rs:247–264 at
30441792a456f98227643624e65ae76620a5fb64). The ordinary order of the closed form should exclude menu rows. Open the Host
menu separately in this fixture, establish both offers, and return to the tab-order setup. Adjacent checks do exercise
the rows by pointer in other fixtures (launcher/environment_view_tests.rs:40–58 and
launcher/sandbox_view_tests.rs:185–230 at 30441792a456f98227643624e65ae76620a5fb64), which covers standalone offer
behavior but not this test's all-placements premise. SPEC_impl.md:9389–9396 at the pinned tip describes the address
picker's non-focusing menu. The reviewer suggested other. Confidence confirmed for the missing premise assertion. No
shipped UI failure is established. Possible coverage: adjacent provider menu tests independently prove row presence in
their fixtures.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-form-checks/LF-5`.
