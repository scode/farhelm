# Window restoration uses the wrong display scale on macOS

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

macOS window restore applies the new display's scale to saved geometry.

## Details

F266 — **definite** — `crates/farhelm-ui/src/desktop/window_state.rs:84` — Window restoration uses the wrong display
scale on macOS

Saved physical position and size reflect the old display's scale, but restoration converts them using the newly created
window's current scale before queuing native moves. When scales differ, valid placement restores incorrectly and can be
saved again in that form. Native results depend on monitor layout; no macOS run occurred. Resolve destination scale
before conversion and persist or resolve sufficient display information, accounting for queued native operations.

## Evidence and triage context

- crates/farhelm-ui/src/desktop.rs:188–192 invokes WindowTracker::attach on the newly created window. :212–223
  configures no saved position or destination display in its builder.
- crates/farhelm-ui/src/desktop/window_state.rs:73–88 validates the saved rectangle, calls set_outer_position with
  PhysicalPosition, then immediately calls set_frame_size.
- crates/farhelm-ui/src/desktop/window_state.rs:339–347 subtracts current decorations and converts saved physical size
  using window.scale_factor().
- Cargo.lock:5477–5478 pins Tao 0.34.8. In tao-0.34.8/src/platform_impl/macos/window.rs:706–713, outer_position
  multiplies logical coordinates by the current window scale; :746–751 does the same for outer size. :728–732 converts a
  requested physical position using the current scale before scheduling the move.
- tao-0.34.8/src/platform_impl/macos/util/async.rs:96–100 queues the already-converted position on the main dispatch
  queue. :87–91 likewise queues the already-converted content size.
- tao-0.34.8/src/platform_impl/macos/window.rs:189–210 selects the initial main-screen geometry when no position is
  supplied, and :331–332 centers the new window. No saved-display scale is available to that initialization.
- crates/farhelm-ui/src/desktop/window_state.rs:155–187 captures and writes the resulting ordinary geometry on close;
  :373–394 obtains that geometry from the current native window.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- {"basis": "SPEC_impl.md:940–952", "comparison": "This describes physical-pixel persistence, monitor validation,
  conversion through the current decoration extent, and a remaining need for native validation. It does not accept
  converting saved coordinates with a different display's scale or restoring to the wrong display."}
- {"basis": "SPEC.md:1355–1359", "comparison": "The explicit placement exception is Wayland. The finding concerns macOS
  with a valid saved rectangle that fits a connected display, where restoration is required."}

Caveats:

- Confirmed by inspection of Farhelm and pinned Tao source, without macOS runtime reproduction.
- Requires the new window's initial display scale to differ from the scale used when saving.
- The precise final placement and size depend on monitor layout and native window constraints; the incorrect conversion
  itself is established.
- No loss of session work is established. The saved window preference can be replaced with the incorrect geometry.
- This can occur on ordinary launches with the affected display setup and can persist in saved state, so FILTER.md:24–42
  does not justify rejection.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_06_cor:p2:F1`.

- `ui_desktop_06_cor:p2:F1`: confidence as filed: definite; suggested bucket as filed: other.
