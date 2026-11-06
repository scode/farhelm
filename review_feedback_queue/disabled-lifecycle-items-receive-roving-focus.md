# Disabled lifecycle items receive roving focus

Reviewed commit: 1a57aa8e265cbb59cedc228433b189f59f7ad59a

## TLDR

Keyboard navigation can target disabled lifecycle actions and lose the intended focus movement.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-ui/src/hosts.rs:4938-4947 at 1a57aa8e265cbb59cedc228433b189f59f7ad59a`.

Source assessment (read with the later corrections below) (review record g058/g058-l5, event 14):

Reviewed commit: 1a57aa8e265cbb59cedc228433b189f59f7ad59a. New source-verified discovery. TLDR: While a Tensorlake host
is suspending or resuming, arrow navigation can aim at an unfocusable Resume command and leave the open host menu with
no enabled Tab entry. Source route: hosts.rs:4416-4433 includes Resume in HostMenuOrder during TensorlakeChanging, but
4938-4947 makes that menu button natively disabled while retaining its normal focus wiring. In a transitioning host's
open menu, Retry is enabled when this viewer has no local operation; ArrowDown includes the disabled Resume slot.
menu_panel.rs:781-820 and 1374-1388 navigate every order slot without a disabled-item filter; focus_menu_item at 901-916
synchronously records the requested disabled index as focused/requested. hosts.rs:4562-4564 and 4941 then put the sole
tabindex=0 on that disabled command, with other items tabindex=-1. Follow-up verification read the actual page helper:
assets/menu-focus.js:95-106 resolves that precise marker and calls target.focus(), returning whether it became active;
it never skips to a different command. Native disabled prevents that focus move; the previous Retry can remain visibly
focused rather than the requested destination, so the initial record's generic 'lose visible focus' wording is narrowed
to failure to advance/invalid tab stop. request_menu_focus at menu_panel.rs:413-449 discards the false result and has no
rollback. A later arrow can recover, so no permanent trap is claimed. Governing SPEC_impl.md:952-953 applies shared
roving focus to host menus; 993-1004 requires unavailable commands to stay focusable with aria-disabled, explicitly
explaining why native disabled breaks navigation. Suggested fix: use aria_disabled for the lifecycle MENU command while
keeping its existing activation refusal; retain ordinary disabled on the separate row Resume button. Suggested bucket
other for keyboard/accessibility degradation; no data/process/credential loss. Confidence confirmed by application
source and the native-disabled focus contract; no dynamic reproduction or engine-specific timing measurement. Temporal:
native disabled already existed for page-busy lifecycle commands; assigned #3048 expands it to routine
lifecycle-changing state while this viewer need not be busy. Pinned tip
8a8c9579e04299410652f0e4681975a7198dfc7b:5007-5018 still has the same native disabled expression. proposed_drop false;
possible_cover unknown, parent handles coverage. No runtime/build/browser/network/process/live-install action.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g058/g058-l5`; supporting
records `g057/c5`. The retained private log preserves earlier wording and its corrections. This record does not claim
runtime reproduction.
