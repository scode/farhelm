//! One session row and its lifecycle, metadata, and menu controls.
//!
//! The row keeps the floating-panel hook state with the component; only the
//! geometry decisions are delegated to the pure menu-panel helpers.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::Session;
#[cfg(test)]
use crate::SessionStatus;
use crate::archive::confirmation as archive_confirmation;
use crate::peer::{DetailPart, PeerLine, display_peer};
use crate::profiles::{existence_word, source_profile_label};
use crate::rename::RenameForm;
use crate::status::{confirm_consequence, status_badge};

use super::menu_panel::{
    PanelPlacement, measurement_outcome, menu_panel_placement_style, should_measure_on_mount,
};
use super::shared::{DeleteTarget, RowState};

/// Which ordinary row controls exist for the current retention state.
///
/// Archive removes terminal lifecycle actions, not metadata management:
/// an archived row can still be opened, renamed, or deleted, but cannot be
/// stopped or archived a second time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RowControlVisibility {
    rename: bool,
    stop: bool,
    archive: bool,
    delete: bool,
}

fn row_control_visibility(archived: bool) -> RowControlVisibility {
    RowControlVisibility {
        rename: true,
        stop: !archived,
        archive: !archived,
        delete: true,
    }
}

/// The session row's class list for its two independent visual states.
///
/// A stale row is DIMMED and badged, never hidden or disabled: SPEC.md
/// requires such sessions to stay listed and be clearly marked, and their
/// lifecycle controls stay live because the helm's refusal (which names
/// the host's state) is a better answer than a dead button. `selected`
/// composes with staleness rather than replacing it — the stale dimming
/// lives on `.session-row-open`'s opacity while the selection highlight is
/// the ROW's background, so a selected stale row shows both truthfully.
/// Static strings per combination, matching the prior two-state shape,
/// rather than a formatted class string.
fn row_class(stale: bool, selected: bool) -> &'static str {
    match (stale, selected) {
        (true, true) => "session-row stale selected",
        (true, false) => "session-row stale",
        (false, true) => "session-row selected",
        (false, false) => "session-row",
    }
}

/// The row menu toggle's accessible name: the session's identity, clamped.
///
/// Every row renders an identical "⋯" toggle, so the NAME is the only
/// thing assistive technology can distinguish the buttons by — an
/// unnamed toggle invites renaming or deleting the wrong session. The
/// clamp exists because a title has no length bound (tens of KB is
/// legal) and an accessible name is read aloud in full; 64 characters
/// is plenty to tell sessions apart, and the ellipsis says something
/// was cut. Char-based, not byte-based, so a multi-byte title can never
/// split a codepoint.
fn menu_label(title: &str) -> String {
    const MAX_CHARS: usize = 64;
    let mut clamped: String = title.chars().take(MAX_CHARS + 1).collect();
    if clamped.chars().count() > MAX_CHARS {
        clamped.truncate(
            clamped
                .char_indices()
                .nth(MAX_CHARS)
                .map_or(clamped.len(), |(i, _)| i),
        );
        format!("session actions for {clamped}…")
    } else {
        format!("session actions for {clamped}")
    }
}

#[cfg(test)]
std::thread_local! {
    // How often the real row component ran in the callback-memoization
    // regression below. Thread-local because each Dioxus virtual DOM is
    // single-threaded while the Rust test harness runs tests concurrently.
    pub(super) static SESSION_ROW_RENDERS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// One row, in two layers. The row itself is a plain `<div>` wrapper
/// around two real `<button>`s — the open button (the session's stacked
/// title/host/cwd/invocation lines) and the small "⋯" actions-menu
/// toggle beside it. Everything else — rename, stop, archive, delete,
/// and their confirm prompts — mounts inside the floating panel the
/// toggle anchors, and only while that panel is open. Real buttons
/// rather than a `div` with `role`/`tabindex`/a hand-rolled `onkeydown`:
/// every action gets Enter- and Space-activation, focus styling, and
/// screen-reader semantics for free (a hand-rolled div also had a latent
/// bug: Space on a focused element scrolls the page unless the handler
/// prevents default, which native button activation never triggers).
///
/// The wrapper is a `div`, not a `button`, because HTML forbids
/// interactive content nested inside a `<button>` — a whole-row button
/// could not legally host the toggle. Tab order follows the layers:
/// closed, it walks open → toggle and on to the next row; open, the
/// panel's controls follow the toggle (rename → stop → archive →
/// delete, as visible per `RowControlVisibility`); confirming, the
/// panel holds consequence text plus confirm → cancel (with initial
/// FOCUS on cancel — see "Focus-on-open" below); renaming, the panel
/// holds the current title plus the field's input → save → cancel.
///
/// ## Host and staleness (PLAN_M6.md item 6)
///
/// The row stacks its fields: the title line leads (with the stale/
/// archived/status badges beside it), then the host, directory, and
/// invocation lines, each ellipsizing alone in the fixed-width sidebar.
/// A row the helm marked stale
/// — its host is in some non-connected state — is dimmed and badged rather
/// than hidden, per SPEC.md's "stay in the list … clearly marked". Its
/// stop/rename/delete controls stay ENABLED, which looks like an oversight
/// and is not: the helm refuses those operations with the host's own state
/// named in the message ("host 3 is unreachable-reprobing, so this operation
/// is refused and nothing was queued: …"), and that message is strictly more
/// useful than a greyed-out button. Nothing queues either way — SPEC.md v1
/// refuses rather than deferring — so there is no risk of a click being
/// quietly banked.
///
/// `data-session-id` stays on the outer wrapper for Playwright to key off
/// of, joined by `data-session-stale` and `data-session-selected` (the
/// latter mirrors the `.selected` highlight — see `RowState::selected` —
/// so e2e can assert selection without matching CSS classes). No
/// `stop_propagation` on the
/// stop/delete buttons: the wrapper
/// `<div>` has no click handler of its own for a click to bubble into
/// (the open action lives on its own sibling button), so there is nothing
/// here for propagation to trigger by accident.
///
/// `error` is this row's own entry from `ListView`'s per-session error map
/// (`None` when its last action succeeded or none has run yet); `busy` is
/// whether a stop/delete for THIS session is currently in flight, which
/// disables both action buttons — both are `ListView`'s per-session state
/// (see its own docs for why a single shared error/in-flight slot would be
/// wrong), just narrowed to this one row by the caller before it gets here.
///
/// `nav_disabled`, unlike `error`/`busy`, is NOT this row's own state — it
/// is `ListView`'s single global "something is in flight somewhere" flag
/// (see `nav_locked` in `ListView`), applied identically to every row's
/// open button. Opening ANY row swaps which session the keyed
/// `SessionView` shows, tearing the previous one down mid-operation, and
/// repaints this list under whatever operation is still in flight — the
/// point of disabling it is to keep the selection still until every
/// in-flight create/stop/delete has delivered its result.
///
/// ## Inline delete confirmation
///
/// `confirming` (also `ListView`'s per-session state, same discipline as
/// `error`/`busy`) swaps the stop/delete pair out for a prompt plus
/// **confirm delete**/**cancel** in place — there used to be a
/// `window.confirm()` call here instead; wry ships no native JS dialogs on
/// macOS's WKWebView (observed directly running the desktop build), which
/// made delete-on-a-live-session silently do nothing on that target. An
/// in-page prompt has no such platform dependency, so it replaces the
/// eval-based one everywhere, not just on desktop. While it is open, tab
/// order walks confirm delete → cancel; initial FOCUS lands directly on
/// cancel regardless of tab order (see below).
///
/// The `.session-row-open` button (title/cwd/invocation/badge) stays
/// VISIBLE while a prompt is pending, merely `disabled`: the prompt lives
/// in the floating actions panel now, which overlays the rows below
/// instead of competing with the button for the row's own space, so the
/// MT-8 overflow that once forced the button out of layout entirely
/// (`display: none` behind a `prompting` class) cannot recur by
/// construction. Disabling it is still required — cancel must be the only
/// way back to normal, never an implicit click on open (see `confirming`
/// in `ListView`).
///
/// The prompt itself is TWO separate elements, not one combined sentence:
/// `.confirm-consequence` (from `confirm_consequence`, fixed wording with
/// no title in it at all) and `.confirm-title` (the title alone, quoted).
/// Both are ordinary Dioxus text interpolation, never `document::eval` —
/// a title containing quotes or JS-source-looking text (plausible, since a
/// supervisor over `--ssh` may be a different, possibly untrusted host)
/// renders as inert DOM text, never something parsed as script, which is
/// what makes the injection concern the old eval path needed
/// `serde_json`-encoding to guard against moot on this path. The SPLIT
/// exists for a different reason: a legal title can be tens of KB with no
/// whitespace at all, and app.css lets `.confirm-title` (only) shrink and
/// ellipsize under space pressure while `.confirm-consequence` never
/// shrinks — so the safety-critical "will be killed" half can never be
/// the one that gets clipped, which a single combined, single-ellipsized
/// string could not promise once the title ran long enough. Rendering the
/// consequence element FIRST is what makes "read the risk before the
/// title" the actual reading order, not just an incidental visual
/// side effect that a later DOM-order change could quietly undo.
///
/// ## Inline rename (PLAN_M5.md item 6)
///
/// `renaming` swaps the actions panel's rename/stop/archive/delete set
/// for the session's own title plus `rename::RenameForm`, disabling (not
/// hiding) the open button exactly as the confirm prompt does. The two
/// states are mutually exclusive by construction — `ListView` refuses to
/// open either while the other is showing — so the branches below can be a
/// plain if/else chain rather than a composition of overlays.
///
/// Repeating the title inside the panel is load-bearing, not decoration:
/// a REFUSED rename must show the rejected draft NEXT TO the name that
/// still stands (SPEC.md requires the old title to stay while the
/// supervisor's refusal is shown), and the row's own title line may be
/// ellipsized past recognition in the narrow sidebar.
///
/// The draft itself is `ListView`'s (`rename_draft`), seeded when the
/// field opens; everything the submitted string then goes through is
/// `ListView`'s too. This component neither validates it nor decides what
/// a refusal means.
///
/// Focus-on-open uses the plain HTML `autofocus` attribute on the cancel
/// button (below), not Dioxus's `onmounted`/`set_focus` API: `set_focus`
/// returns a `Result` future that can fail (`MountedError`, e.g. on a
/// renderer that does not support it), and since focus-on-cancel is a
/// safety default — landing keyboard focus on the SAFE action before a
/// stray Enter/Space can reach anything — silently discarding that
/// `Result` would let the safety behavior vanish with nothing to show for
/// it. `autofocus` cannot fail the same way: it is applied by the browser
/// itself at parse/insert time as a plain attribute, with no fallible
/// async call in the UI's own control to get wrong or ignore. It fires
/// whenever the cancel button is freshly created inside the `if
/// confirming` branch — which, since the prompt lives in the on-demand
/// panel, can be MORE than once per confirmation: the confirming flag
/// survives the panel closing (see `confirming` in `ListView`), so
/// closing and reopening the panel remounts the prompt and lands focus
/// on cancel again. That repeat is the safe direction — every fresh
/// appearance of the prompt starts with the escape hatch focused.
#[component]
#[allow(clippy::too_many_arguments)]
pub(super) fn SessionRow(
    session: Session,
    state: RowState,
    rename_draft: Signal<String>,
    on_open: EventHandler<Session>,
    on_stop: EventHandler<String>,
    on_delete: EventHandler<DeleteTarget>,
    on_confirm_delete: EventHandler<String>,
    on_cancel_delete: EventHandler<String>,
    on_archive: EventHandler<Session>,
    on_confirm_archive: EventHandler<String>,
    on_cancel_archive: EventHandler<String>,
    on_rename_start: EventHandler<(String, String)>,
    on_rename_submit: EventHandler<(String, String)>,
    on_rename_cancel: EventHandler<()>,
    on_menu_toggle: EventHandler<String>,
) -> Element {
    let RowState {
        error,
        busy,
        confirming,
        confirming_archive,
        renaming,
        nav_disabled,
        menu_open,
        selected,
    } = state;
    #[cfg(test)]
    SESSION_ROW_RENDERS.with(|renders| renders.set(renders.get() + 1));
    // `None` for a status nothing has classified yet, and the row then
    // renders no badge ELEMENT at all rather than an empty one — see
    // `status_badge`'s own docs for why an empty badge box would be the
    // same mistake in CSS.
    let badge = status_badge(&session.status, session.annotation.as_deref());
    let open_session = session.clone();
    let stop_id = session.id.clone();
    let delete_target = DeleteTarget {
        id: session.id.clone(),
        status: session.status.clone(),
    };
    let archive_target = session.clone();
    let confirm_id = session.id.clone();
    let cancel_id = session.id.clone();
    let confirm_archive_id = session.id.clone();
    let cancel_archive_id = session.id.clone();
    let rename_start = (session.id.clone(), session.title.clone());
    let rename_submit_id = session.id.clone();
    let controls = row_control_visibility(session.archived);
    let menu_id = session.id.clone();
    // The toggle's own `MountedData`, captured once via `onmounted` below,
    // and where the panel believes its own screen position currently is
    // (`PanelPlacement` — its own doc has the state machine) — both
    // row-local (unlike `menu_open`, which `ListView` owns so only one
    // row's menu can ever be open). `ListView` has no business knowing
    // this row's screen geometry, and this row has no business deciding
    // WHETHER its menu is open — the split mirrors that division exactly.
    let mut toggle_handle = use_signal(|| None::<Rc<MountedData>>);
    let mut placement = use_signal(|| PanelPlacement::Unmeasured);
    // Bumped on every FRESH open (the toggle's `onclick`, opening branch
    // only — never on close, and never by the `onmounted` heal path,
    // which reuses whatever generation is already current rather than
    // starting a new one). A still-in-flight measurement from an OLDER
    // open captures its own generation before awaiting, and discards its
    // result if this counter has since moved on — see `spawn_measurement`
    // below. This closes a race a plain "reset placement, then measure"
    // scheme leaves open: two opens of the SAME toggle close enough
    // together (a fast close-then-reopen, or reopen after the row itself
    // moved) can have their measurements resolve out of order, and
    // without this counter the OLDER one finishing LAST would silently
    // overwrite the newer, correct measurement with a stale one.
    let mut open_generation = use_signal(|| 0_u64);
    // Shared by the toggle's `onclick` (every fresh open) and its
    // `onmounted` (healing a remount that lands with the menu ALREADY
    // open — see that handler's own doc for why that happens and why
    // re-measuring there is what fixes it). Takes no arguments: both call
    // sites close over this row's own `toggle_handle`/`placement`/
    // `open_generation` directly, and both want the exact same
    // capture-generation → query → classify-if-still-current sequence, so
    // this is the one place that sequence is written.
    let spawn_measurement = move || {
        let handle = toggle_handle;
        let mut placement = placement;
        // Captured SYNCHRONOUSLY, before the `await` below yields — a
        // generation bump landing after this line but before the read
        // would otherwise be invisible to this task, defeating the guard
        // it exists to provide.
        let generation = open_generation();
        spawn(async move {
            let measured = match handle.peek().clone() {
                Some(handle) => handle.get_client_rect().await.ok(),
                None => None,
            };
            // `measurement_outcome` (in `menu_panel`) is the apply-if-current
            // decision itself, pinned by its own unit test; this call is
            // just where the real async race it guards against actually
            // happens. See `open_generation`'s own doc for the concrete
            // interleaving a newer open superseding this one guards
            // against.
            if let Some(outcome) =
                measurement_outcome(generation, *open_generation.peek(), measured)
            {
                placement.set(outcome);
            }
        });
    };
    let row_class = row_class(session.stale, selected);

    rsx! {
        div {
            class: row_class,
            "data-session-id": "{session.id}",
            "data-session-stale": "{session.stale}",
            "data-session-archived": "{session.archived}",
            "data-session-selected": "{selected}",
            // Two stacked rows, not one: the buttons need a plain flex
            // ROW (see `.session-row-main` in app.css), but a per-session
            // error line needs its own full-width row underneath rather
            // than squeezing in as a fourth flex item next to the
            // buttons — hence the extra wrapper rather than putting
            // everything directly under `.session-row`.
            div { class: "session-row-main",
                button {
                    r#type: "button",
                    class: "session-row-open",
                    // The accessible counterpart of the visual highlight:
                    // the sidebar is a navigation-shaped list of open
                    // buttons, and `aria-current` is the native way to say
                    // "this one is where you are" without inventing a
                    // listbox role for a list that is not one. Absent
                    // entirely on unselected rows (a conditional
                    // attribute), not `"false"`.
                    aria_current: if selected { "true" },
                    // Disabled by ANY of the three locks: the global nav
                    // lock (any in-flight op anywhere), or this row's own
                    // confirmation or rename field being open — the
                    // simplest way to satisfy "cancel is the only way back
                    // to normal" (see the component doc above) is to make
                    // the open button inert for the whole time a prompt is
                    // showing, rather than giving it a second, competing
                    // meaning as an implicit cancel.
                    disabled: nav_disabled || confirming || confirming_archive || renaming,
                    onclick: move |_| on_open.call(open_session.clone()),
                    // STACKED lines rather than one squeezed flex row: the
                    // sidebar column (BUGS_BURNDOWN.md issue 5, interviewed
                    // row contents) is far too narrow for the old
                    // everything-on-one-line layout, whose min-width floors
                    // produced the MT-8 overflow class the moment space ran
                    // short. The title line leads with identity and its
                    // qualifiers; host, cwd, and invocation each get a line
                    // they can ellipsize alone.
                    // `span` wrappers, not `div`: this all sits inside the
                    // native `.session-row-open` <button>, whose content
                    // model only permits phrasing content — a flow-content
                    // div inside a button is invalid HTML that engines and
                    // accessibility tooling may interpret inconsistently.
                    // The stacked-line layout comes from the class's CSS,
                    // not the element kind.
                    span { class: "session-row-line",
                        span { class: "session-title", "{session.title}" }
                        // Beside the status badge rather than replacing it:
                        // the last-known status is still what the helm
                        // knows, and this says how old that knowledge is —
                        // two facts, not one, exactly as the stop annotation
                        // qualifies rather than replaces `exited`.
                        if session.stale {
                            span { class: "stale-badge", "stale" }
                        }
                        if session.archived {
                            span { class: "archived-badge", "archived" }
                        }
                        if let Some((badge_class, badge_text)) = badge {
                            span { class: "status-badge {badge_class}", "{badge_text}" }
                        }
                    }
                    // The host gets the second line: with more than one
                    // machine in play it is what disambiguates two otherwise
                    // identical sessions. The name is the helm's own
                    // rendering (`host_name`), denormalized onto the row so
                    // the list needs no second request — a row from a helm
                    // that sends none simply shows nothing here rather than
                    // inventing a label. Escaped and direction-isolated like
                    // every other rendering of a destination: this one names
                    // the machine a row's stop and delete will reach, so a
                    // name able to reorder the line around it could make one
                    // host's session read as another's.
                    if let Some(host_name) = &session.host_name {
                        span { class: "session-row-line",
                            span { class: "session-host peer-value", dir: "ltr",
                                "{display_peer(host_name)}"
                            }
                        }
                    }
                    span { class: "session-row-line",
                        // Two spans, not one: `.session-cwd` is the rtl
                        // clipping container that puts the ellipsis on the
                        // LEFT, and the inner `dir="ltr"` child is the bidi
                        // isolate that keeps the path's characters in
                        // logical order under it — rtl applied directly to
                        // the text would move a leading "/" to the visual
                        // right (see `.session-cwd` in app.css).
                        span { class: "session-cwd",
                            span { class: "session-cwd-text", dir: "ltr", "{session.cwd}" }
                        }
                    }
                    // No profile chip on the line (the interviewed row
                    // contents): the source-profile surface lives in the
                    // actions menu panel below.
                    span { class: "session-row-line",
                        span { class: "session-invocation", "{session.invocation}" }
                    }
                }
                // The actions menu: one small toggle beside the open
                // button, everything else in a floating panel it anchors
                // (BUGS_BURNDOWN.md issue 5's interviewed design — the row
                // itself carries no action buttons). The panel is also
                // where a destructive action CONFIRMS: clicking delete or
                // archive swaps the panel's contents for the consequence
                // line and confirm/cancel pair, keeping the whole exchange
                // on one small surface instead of bouncing the user
                // somewhere else. Rename lives here too — the ONLY
                // rename surface, by decision, which is what retires the
                // old dual-optimistic-overlay disagreement with the
                // titlebar (whose affordance the redesign removed).
                // Deliberately NOT disabled under the nav lock: opening a
                // panel mutates nothing — every action inside it carries
                // its own disabled state — and locking the toggle would
                // hide the very buttons whose disabled state tells the
                // user WHY the page is briefly inert.
                button {
                    r#type: "button",
                    class: "btn session-row-menu",
                    // The session's identity is part of the accessible
                    // name: a list renders one of these per row, and
                    // "session actions" alone leaves a screen-reader user
                    // no way to tell which session a toggle controls
                    // before activating it. Clamped, because a title can
                    // legally run to tens of KB and an accessible name is
                    // read aloud in full.
                    aria_label: menu_label(&session.title),
                    aria_expanded: menu_open,
                    // Fires once this button is actually in the DOM, giving
                    // us a handle `get_client_rect()` can be called on
                    // later. Also covers a rarer case: a FAILED listing
                    // read unmounts every row (`ListView`'s render swaps to
                    // an error banner) without clearing `menu_open` — that
                    // flag is `ListView`'s, and a transient read failure is
                    // not evidence the user changed their mind — so a row
                    // that remounts on recovery can land here with
                    // `menu_open` already true and `placement` back at its
                    // freshly-initialized `Unmeasured` (a brand new
                    // row-local signal). Nothing else would ever start a
                    // measurement for that panel — the click that opened it
                    // is long gone — so it would stay invisible forever
                    // without this: healing it right here, the instant the
                    // handle FIRST becomes usable, is what a plain "measure
                    // once on open" scheme misses on this one path.
                    onmounted: move |element| {
                        toggle_handle.set(Some(element.data()));
                        if should_measure_on_mount(menu_open, *placement.peek()) {
                            spawn_measurement();
                        }
                    },
                    onclick: move |_| {
                        // `on_menu_toggle` fires FIRST and synchronously,
                        // before any `await` — this is what keeps
                        // `aria_expanded` truthful the instant the click
                        // lands and keeps a fast double-click (or two
                        // clicks landing close together on different rows)
                        // from racing an async measurement into a stray
                        // extra toggle. The panel mounts hidden
                        // (`PanelPlacement::Unmeasured`) the same render —
                        // see that variant's own doc — so opening no longer
                        // needs to WAIT for a measurement, only closing
                        // still needs none at all.
                        let opening = !menu_open;
                        on_menu_toggle.call(menu_id.clone());
                        if !opening {
                            return;
                        }
                        // A fresh measurement every open: the toggle can
                        // move BETWEEN opens (a row above it changing
                        // height, the window resizing), so a rect measured
                        // for a previous open is not safe to reuse — hence
                        // resetting to `Unmeasured` here rather than
                        // leaving whatever `placement` last held. The
                        // generation bump BEFORE `spawn_measurement` is
                        // what keeps a measurement still in flight for a
                        // PRIOR open of this SAME toggle from landing after
                        // this reset and clobbering it — see
                        // `open_generation`'s own doc for the exact race.
                        open_generation += 1;
                        placement.set(PanelPlacement::Unmeasured);
                        spawn_measurement();
                    },
                    "⋯"
                }
                if menu_open {
                    div {
                        class: "session-row-menu-panel",
                        // The presentation half of `PanelPlacement`'s state
                        // machine — see that type's own doc for what each
                        // variant means and why the panel needs three
                        // states rather than a plain measured/unmeasured
                        // flag.
                        style: menu_panel_placement_style(placement()),
                        if confirming {
                            // Two elements, consequence first: an
                            // untruncatable consequence and a separately
                            // truncatable title, in THIS order, so a long
                            // title can never clip the safety-critical
                            // half (see the component doc above).
                            span {
                                class: "confirm-consequence",
                                "{confirm_consequence(&session.status)}"
                            }
                            span { class: "confirm-title", "\"{session.title}\"" }
                            button {
                                r#type: "button",
                                class: "btn confirm-delete",
                                // Disabled while the shared token is held:
                                // the handler refuses then anyway (keeping
                                // the prompt), and the attribute is that
                                // refusal made visible. Cancel stays
                                // enabled — backing out is always safe.
                                disabled: busy,
                                onclick: move |_| on_confirm_delete.call(confirm_id.clone()),
                                "confirm delete"
                            }
                            button {
                                r#type: "button",
                                class: "btn confirm-cancel",
                                // Safe default: land keyboard focus on
                                // cancel, not confirm, the instant this
                                // prompt appears — a stray Enter/Space
                                // right after the menu's delete click backs
                                // OUT of the destructive action instead of
                                // into it (see the component doc for why
                                // declarative `autofocus` over the async
                                // focus API).
                                autofocus: true,
                                onclick: move |_| on_cancel_delete.call(cancel_id.clone()),
                                "cancel"
                            }
                        } else if confirming_archive {
                            if let Some(consequence) = archive_confirmation(&session, session.tabs.len()) {
                                span { class: "confirm-consequence", "{consequence}:" }
                            } else {
                                span { class: "confirm-consequence", "archiving removes the terminal:" }
                            }
                            span { class: "confirm-title", "\"{session.title}\"" }
                            button {
                                r#type: "button",
                                class: "btn confirm-archive",
                                // See confirm-delete: refusal made visible.
                                disabled: busy,
                                onclick: move |_| on_confirm_archive.call(confirm_archive_id.clone()),
                                "confirm archive"
                            }
                            button {
                                r#type: "button",
                                class: "btn archive-cancel",
                                autofocus: true,
                                onclick: move |_| on_cancel_archive.call(cancel_archive_id.clone()),
                                "cancel"
                            }
                        } else if renaming {
                            // The AUTHORITATIVE title stays beside the
                            // field: a refused rename must never leave the
                            // rejected DRAFT as the only name on this
                            // surface (SPEC.md's "the old title stays"
                            // while the refusal is shown).
                            span { class: "rename-current-title", "{session.title}" }
                            RenameForm {
                                draft: rename_draft,
                                busy,
                                on_submit: move |title| {
                                    on_rename_submit.call((rename_submit_id.clone(), title))
                                },
                                on_cancel: move |_| on_rename_cancel.call(()),
                            }
                        } else {
                            if controls.rename {
                                button {
                                    r#type: "button",
                                    class: "btn session-row-rename",
                                    disabled: busy,
                                    onclick: move |_| on_rename_start.call(rename_start.clone()),
                                    "rename"
                                }
                            }
                            if controls.stop {
                                button {
                                    r#type: "button",
                                    class: "btn session-row-stop",
                                    disabled: busy,
                                    onclick: move |_| on_stop.call(stop_id.clone()),
                                    "stop"
                                }
                            }
                            if controls.archive {
                                button {
                                    r#type: "button",
                                    class: "btn session-row-archive",
                                    disabled: busy,
                                    onclick: move |_| on_archive.call(archive_target.clone()),
                                    "archive"
                                }
                            }
                            if controls.delete {
                                button {
                                    r#type: "button",
                                    class: "btn session-row-delete",
                                    disabled: busy,
                                    onclick: move |_| on_delete.call(delete_target.clone()),
                                    "delete"
                                }
                            }
                            // The profile this session was CREATED from, as
                            // it snapshotted the name — moved here from the
                            // row proper (the interviewed row contents drop
                            // the chip) so SPEC.md's snapshot rule keeps a
                            // visible surface: the name never moves under
                            // an existing session, and the qualifier
                            // (`profiles::source_profile_label`) keeps that
                            // from reading as a claim about today's
                            // catalog. `data-profile-existence` remains the
                            // browser suite's handle on the half that does
                            // change.
                            if let Some(source) = &session.source_profile {
                                span {
                                    class: "session-profile peer-value",
                                    dir: "ltr",
                                    "data-profile-existence": "{existence_word(source.existence)}",
                                    "{source_profile_label(source)}"
                                }
                            }
                        }
                    }
                    // The refusal a panel action produced renders INSIDE
                    // the open panel, under its controls: the panel floats
                    // over the row's own error line, so an error rendered
                    // only down there could sit hidden behind the very
                    // surface whose click caused it.
                    if let Some(err) = error.clone() {
                        PeerLine {
                            class: "action-error".to_string(),
                            parts: vec![DetailPart::Peer(err)],
                        }
                    }
                }
            }
            // The helm's own refusal, and on a stale row it names the host's
            // state and can quote peer-supplied text — so it renders through
            // the same escaping and isolation as every other peer string.
            // Only while the panel is CLOSED: the open panel carries the
            // error itself (above), and rendering both would say one
            // refusal twice.
            if let Some(err) = error {
                if !menu_open {
                    PeerLine {
                        class: "action-error".to_string(),
                        parts: vec![DetailPart::Peer(err)],
                    }
                }
            }
        }
    }
}

/// One session as the row render tests below need it: real enough for
/// `SessionRow` to render, stable so that reconstructing it every
/// parent render compares equal and memoization is what is measured.
#[cfg(test)]
pub(super) fn row_specimen(id: &str) -> Session {
    Session {
        id: id.to_string(),
        title: "stable".to_string(),
        cwd: "/tmp".to_string(),
        invocation: "agent".to_string(),
        status: SessionStatus::Exited { exit_code: Some(0) },
        annotation: None,
        restart_offer: crate::RestartOffer::FreshOnly,
        created_at: 0,
        archived: false,
        tabs: Vec::new(),
        host: None,
        host_identity: None,
        host_name: None,
        stale: false,
        source_profile: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Revealing an archived row must keep its metadata actions while
    /// withholding lifecycle controls that no longer have a terminal to act
    /// on.
    #[test]
    fn archived_rows_keep_metadata_controls_without_lifecycle_controls() {
        assert_eq!(
            row_control_visibility(true),
            RowControlVisibility {
                rename: true,
                stop: false,
                archive: false,
                delete: true,
            }
        );
    }

    /// Repeated parent refreshes must update direct callback props in place
    /// without rerendering an otherwise unchanged session row.
    ///
    /// Dioxus gives `EventHandler` component props special ownership and
    /// memoization. Hiding them inside an ordinary props struct bypasses
    /// that path: every fleet refresh then retains another callback set and
    /// rerenders every row. This drives the real `SessionRow` through many
    /// parent renders, so either regression changes the count from one.
    #[test]
    fn repeated_parent_refreshes_do_not_rerender_an_unchanged_row() {
        fn app() -> Element {
            let rename_draft = use_signal(String::new);
            let on_open = use_callback(|_: Session| {});
            let on_stop = use_callback(|_: String| {});
            let on_delete = use_callback(|_: DeleteTarget| {});
            let on_confirm_delete = use_callback(|_: String| {});
            let on_cancel_delete = use_callback(|_: String| {});
            let on_archive = use_callback(|_: Session| {});
            let on_confirm_archive = use_callback(|_: String| {});
            let on_cancel_archive = use_callback(|_: String| {});
            let on_rename_start = use_callback(|_: (String, String)| {});
            let on_rename_submit = use_callback(|_: (String, String)| {});
            let on_rename_cancel = use_callback(|_: ()| {});
            let on_menu_toggle = use_callback(|_: String| {});
            let session = row_specimen("session-1");
            rsx! {
                SessionRow {
                    session,
                    state: RowState {
                        error: None,
                        busy: false,
                        confirming: false,
                        confirming_archive: false,
                        renaming: false,
                        nav_disabled: false,
                        menu_open: false,
                        selected: false,
                    },
                    rename_draft,
                    on_open,
                    on_stop,
                    on_delete,
                    on_confirm_delete,
                    on_cancel_delete,
                    on_archive,
                    on_confirm_archive,
                    on_cancel_archive,
                    on_rename_start,
                    on_rename_submit,
                    on_rename_cancel,
                    on_menu_toggle,
                }
            }
        }

        SESSION_ROW_RENDERS.with(|renders| renders.set(0));
        let mut dom = VirtualDom::new(app);
        dom.rebuild_to_vec();
        for _ in 0..64 {
            dom.mark_dirty(dioxus::core::ScopeId::APP);
            dom.render_immediate(&mut dioxus::core::NoOpMutations);
        }
        SESSION_ROW_RENDERS.with(|renders| {
            assert_eq!(
                renders.get(),
                1,
                "unchanged rows must stay memoized across fleet refreshes"
            );
        });
    }

    /// A selection switch rerenders exactly the two rows whose `selected`
    /// flag changed; every other row stays memoized.
    ///
    /// This is the cost model the selection highlight was designed to: the
    /// flag participates in prop comparison (via `RowState`'s `PartialEq`,
    /// though any compared prop position would do), so a selection change
    /// is two row renders, not a fleet-wide repaint. If selection ever
    /// stops being part of what memoization compares — or starts invalidating
    /// rows it does not describe — this count drifts from 2 and catches it.
    #[test]
    fn a_selection_change_rerenders_only_the_affected_rows() {
        // The selection lives OUTSIDE the virtual DOM (the test flips it
        // between renders, the way `AppBody`'s signal changes between
        // `ListView` renders); the app below re-derives every `RowState`
        // from it on each parent render, so memoization alone decides
        // which rows actually run.
        std::thread_local! {
            static SELECTED: std::cell::Cell<&'static str> = const { std::cell::Cell::new("session-1") };
        }

        fn app() -> Element {
            let rename_draft = use_signal(String::new);
            let on_open = use_callback(|_: Session| {});
            let on_stop = use_callback(|_: String| {});
            let on_delete = use_callback(|_: DeleteTarget| {});
            let on_confirm_delete = use_callback(|_: String| {});
            let on_cancel_delete = use_callback(|_: String| {});
            let on_archive = use_callback(|_: Session| {});
            let on_confirm_archive = use_callback(|_: String| {});
            let on_cancel_archive = use_callback(|_: String| {});
            let on_rename_start = use_callback(|_: (String, String)| {});
            let on_rename_submit = use_callback(|_: (String, String)| {});
            let on_rename_cancel = use_callback(|_: ()| {});
            let on_menu_toggle = use_callback(|_: String| {});
            let selected = SELECTED.with(|selected| selected.get());
            rsx! {
                for id in ["session-1", "session-2", "session-3"] {
                    SessionRow {
                        key: "{id}",
                        session: row_specimen(id),
                        state: RowState {
                            error: None,
                            busy: false,
                            confirming: false,
                            confirming_archive: false,
                            renaming: false,
                            nav_disabled: false,
                            menu_open: false,
                            selected: selected == id,
                        },
                        rename_draft,
                        on_open,
                        on_stop,
                        on_delete,
                        on_confirm_delete,
                        on_cancel_delete,
                        on_archive,
                        on_confirm_archive,
                        on_cancel_archive,
                        on_rename_start,
                        on_rename_submit,
                        on_rename_cancel,
                        on_menu_toggle,
                    }
                }
            }
        }

        SELECTED.with(|selected| selected.set("session-1"));
        SESSION_ROW_RENDERS.with(|renders| renders.set(0));
        let mut dom = VirtualDom::new(app);
        dom.rebuild_to_vec();
        SESSION_ROW_RENDERS.with(|renders| {
            assert_eq!(renders.get(), 3, "initial build renders every row once");
        });

        SELECTED.with(|selected| selected.set("session-2"));
        dom.mark_dirty(dioxus::core::ScopeId::APP);
        dom.render_immediate(&mut dioxus::core::NoOpMutations);
        SESSION_ROW_RENDERS.with(|renders| {
            assert_eq!(
                renders.get(),
                5,
                "a selection switch must rerender the deselected and newly \
                 selected rows and nothing else"
            );
        });
    }

    /// `stale` and `selected` are independent row states and every
    /// combination must say so in the class list — in particular a
    /// selected STALE row carries both, because selection must not hide
    /// the SPEC.md-required stale marking and staleness must not hide
    /// which session the main pane is on.
    #[test]
    fn row_class_composes_stale_and_selected_independently() {
        assert_eq!(row_class(false, false), "session-row");
        assert_eq!(row_class(true, false), "session-row stale");
        assert_eq!(row_class(false, true), "session-row selected");
        assert_eq!(row_class(true, true), "session-row stale selected");
    }
}
