# Pi still lacks an independent successful Settings read and no-hint assertion

Reviewed commit: 4949a79e8465ce1f25e0e5c42a1e76b80730ce0b

## TLDR

Pi's Settings tests no longer check a successful trust read or the absence of a hook hint control.

## Details

**other.** Source inspection.

4949a79e8465ce1f25e0e5c42a1e76b80730ce0b crates/gui/farhelm-gui/src/settings/tests.rs:2178–2217,2347–2385;
1a53d9d35889d7dde96e31320cc1681dd013737d crates/gui/farhelm-gui/src/settings/tests.rs:2187–2225,2405–2438 and
settings/search/tests.rs:493–535

A future regression can make Pi's Settings page show the wrong stored trust choice or a misleading hint switch without
the changed GUI assertions catching it. No such shipped product failure was found. At pinned tip
1a53d9d35889d7dde96e31320cc1681dd013737d both Agents navigation and search call show for Pi (settings/frame.rs:309–314;
settings/search.rs:338–357,769–778,809–812; settings.rs:1138–1166). First display reads the selected agent once and
applies its reply to that slot (settings.rs:1006–1043); the switch uses the same Pi-specific path for saves
(settings.rs:1548–1573; settings/agent_trust.rs:142–149,366–380). Pi has no hint choice or recognized session trust
question (settings/agent_trust.rs:279–312), so a session hint is not a Pi Settings entry point. The parent
d04f63f48a2b4a8eecf7c5071b339198a1483896 settings/tests.rs:2118–2125,2153–2184,2312–2333 checked all trust GET paths,
each switch's state after success, and absence of a specifically named Pi hint ID. The old negative assertion was
limited to that hypothetical ID, so it did not establish that a borrowed hint ID could never appear. The successor
visits selected pages. The catalog check visits Pi and checks presence and order for whatever the shared catalog
expects, but replies 503 to its read and never checks for extra controls. The helm router's independent test explicitly
exercises Pi GET, PUT and reply fields (crates/farhelm-helm/src/checkout_config.rs:592–673); storage and compiled Pi
arguments are covered separately (checkout_config.rs:774–819,858–864; launches.rs:2731–2761). Session-pane tests pin
Pi's lack of a band and trust request (crates/gui/farhelm-gui/src/session_pane/trust_band_tests.rs:754–789,918–958). No
companion scene shows Pi (goldens/mod.rs:1187–1210). The remaining gap is the link from the supported Settings
interaction to the successful Pi read and expected visible controls. A focused assertion should pin Pi's GET, respond
with its own enabled value, and check the single trust switch and absence of any hint control. The reviewer suggested
other, test coverage. Confidence confirmed from source, runtime unverified.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`agent-settings-checks/trust-check-coverage`.
