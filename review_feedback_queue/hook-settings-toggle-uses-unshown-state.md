# A Space press can save the all-hosts hook opt-in from an unpresented switch state

Reviewed commit: bb31959c2f22c96bbbb72da2e60eee4a0b288a6b

## TLDR

Space can save the all-hosts hook opt-in while the switch still shows the on state the user intended to turn off.

## Details

**highest.** Source inspection.

bb31959c2f22c96bbbb72da2e60eee4a0b288a6b crates/gui/farhelm-gui/src/settings/hook_settings.rs:44-50;
crates/gui/farhelm-gui/src/components/checkbox.rs:223-228,317-349;
crates/gui/farhelm-gui/src/settings.rs:1175-1197,1575-1590;
crates/gui/farhelm-gui/src/settings/configured.rs:19-34,91-115; vendor/gpui-pre/src/window.rs:3849,3887-3901,6503-6509;
crates/farhelm-helm/src/lib.rs:746-752,939-945; crates/farhelm-helm/src/agent_hook_setup.rs:32-36,52-73,106-126;
crates/farhelm-helm/src/sessions.rs:2179-2192; crates/farhelm-supervisor/src/service/core.rs:14758-14777

A Space press intended to turn off persistent hook setup can instead save the all-hosts opt-in. The checkbox passes the
inverse of the state drawn for keyboard dispatch, and the Settings closure sends that boolean for its captured agent.
GPUI draws a dirty frame before selecting the key handler and flags it for a distinct later presentation. A refused
enable clears saving and restores the stored off value while the visible frame can still show on. Operations frees the
preferences key before the callback settles the draft, and the switch remains focusable
(drafts.rs:106-112,131-137,161-172 and operations.rs:886-894,900-923 at the reviewed commit). The route authenticates
the device session and validates the vendor. Its body has only enabled, and the full-sync store update uses that boolean
directly, with no presented-frame evidence, expected prior value, or separate confirmation. The helm takes the saved
value into creates. The supervisor gates installation by the request and host hook policy, which limit where a write
takes effect and do not validate the gesture that saved consent. SPEC.md at
1a53d9d35889d7dde96e31320cc1681dd013737d:3085-3090,5572-5577 and SPEC_impl.md:973-988,1033-1035 require presented
consent. Bind vendor and target boolean to a presented action snapshot or ignore the key until the changed state is
shown. Confirmed by source, with the relevant GUI, route, and GPUI files unchanged through the pinned tip. Incidence is
unmeasured. Durable opt-in requires the indicated interleaving and an accepted write, and an installation also requires
a subsequent eligible start. The reviewer suggested highest for the source-supported authorization error and persistent
vendor configuration consequence.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `hook-settings/HS-5`.
