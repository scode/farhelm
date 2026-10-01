# The YOLO guard allows a launch when the host row is missing

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

In a narrow window while a host is being removed, a YOLO launch to that host can go through without the sensitive-host
confirmation, because the guard treats a missing host record as "allowed".

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F26 / COR-YOLO-MISSING-ROW`, tagged **possible**. Anchor and title: `crates/farhelm-helm/src/yolo_guard.rs:88`
— the YOLO guard lets a launch through when the host's registry row is missing.

Every host is "sensitive" for YOLO launches until the user marks it safe. The helm's YOLO guard
(`crates/farhelm-helm/src/yolo_guard.rs`) refuses an approval-skipping launch on a sensitive host unless the request
carries the explicit override. It decides by looking up the host's row in the helm's host registry at the moment of the
check. If no row is found, it returns OK (`yolo_guard.rs:88-90`), with a comment that "routing to it refuses on its
own".

That comment assumes the guard runs before routing, but it does not. The create path (`sessions.rs:1710`) and
restart-with (`sessions.rs:2714`) have already routed the request and hold a live connection client for the host when
they call the guard. Host removal (`hosts.rs:786-790`) deletes the registry row first and only then stops the host's
connection actor (the per-host task that holds the supervisor connection). A YOLO create that was routed just before the
removal and reaches the guard between those two steps finds no row, is allowed, and is dispatched over the still-open
connection without any confirmation.

The window is narrow, and it requires the connection to still be usable during it, hence possible. But this is the one
place where a guard designed to fail closed fails open. Suggested change: treat a missing row as sensitive (refuse with
the usual YOLO-on-sensitive-host error) or as host-not-found, and correct the comment.
