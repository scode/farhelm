//! The open session's details bar: the slim row under its terminal that names the session, with its status, title,
//! host, age, managed-checkout mark, directory and command line, in that order.
//!
//! SPEC.md, "Terminal experience": "The bottom bar is about one tab-strip row high and shows, in order, status, session
//! name, host, age, the managed-checkout branch mark when present, directory, and command line." Ported from the old
//! app's `.session-details` (`session_view.rs` and `app.css` at the reference commit).
//!
//! Here's the TLDR of the rules:
//!
//! - **The list's own words and marks.** The status is the session row's badge ([`status::live`] and
//!   [`status::ended_words`], the plain `Exited` form the quick switcher and the menu header use), without the row's
//!   seen toggle: only a row's own dot is a control. The host mark is the row's ([`row_marks::HostMark::of`]): none when
//!   the session's host cannot be placed, and a hidden `Local` or `Remote` as its name. The age is the row's
//!   ([`age::of`]), with the full stamp as hover help. A second copy of any of these would let the bar describe a
//!   session differently from its row.
//! - **The host is the helm's full name**, never the row's short one, as the session sent it ([`identity::named`]): the
//!   local host under the registry's own name reads `Local (this machine)`, and a session that names no host shows none
//!   rather than an invented one. A session in its own environment names it as its row does ("dev sandbox"), with its
//!   provider's mark for the host mark (`session_list::environment`), and while that environment is suspended the bar
//!   says "Suspended" with the pause glyph where the age would be, as the old bar did. While it is being deleted, the
//!   bar shows the host's phase words there instead ("Deleting…" or "Deleting failed"), helped by why the session's
//!   actions are unavailable, as the row does, and no age, as the old bar's `if … else if` chain drew one of the three.
//! - **What a status cannot claim, it leaves out.** An unclassified session has no badge but keeps its age (SPEC.md,
//!   "Status": a session nothing has classified "shows no status and still shows its age"). A stale session (its host is
//!   not connected) has neither: its last-known status and age belong in the stale band, and SPEC.md says
//!   "a stale session's status appears only in its stale notice, not again in the bottom bar".
//! - **Who gives way.** The bar never wraps. A live mark, the host mark and the age never shrink. The ended words, the
//!   title and the host name shrink as the old bar's flex items did (`flex: 0 1 auto; min-width: 0`, GPUI's default
//!   shrink with the minimum taken off), so each gives up width in proportion to its own: a long title gives up the
//!   most, and in a narrow pane the title and the host are cut together, as the old narrow screens show. Each ends in an
//!   ellipsis when cut, with its whole value as hover help. The copy fields give way before any of them, as SPEC.md
//!   says ("ellipsize before the session name or host when the bar runs out of room"): the two sit in one box that
//!   starts from nothing and takes only the room the other parts leave ([`copy_fields`]), so the title and the host
//!   are cut only once the fields have nothing left to give. The old bar's `flex: 0 3 auto` alone did not do that:
//!   flex shrinking is weighted by width, so beside a long command the old title was cut too.
//! - **The directory and the command line copy themselves.** Each is a [`CopyField`]: the value drawn as itself in the
//!   monospace face, and a click copies the raw recorded value and says "✓ Copied" for a moment. The directory is
//!   shown with a personal home folded to `~`, as the session list folds it ([`abbreviate_home`]), except when the
//!   folded-away account segment holds a presentation-unsafe character: the fold would then hide the escape of a
//!   character the copy holds. Hover help, accessible description and the copied bytes keep the raw value, as
//!   "{value} — click to copy" with the value escaped. A value that would show nothing shows the old marker instead
//!   (`(empty)`, or `(whitespace only: N characters)`; [`blank_placeholder`]), so neither field is ever an unnamed sliver of
//!   a button.
//! - **A copy that holds hidden characters says so above the bar.** When the bytes a click copied hold a character
//!   the screen shows escaped (`<U+202E>`, a line break), the clipboard does not hold what the field reads like, and a
//!   command pasted into a shell runs the clipboard's version. So while that click's confirmation shows, a warning
//!   opens upward from the bar, outside the fields, which shrink and could clip it: "⚠ The copied command contains
//!   hidden characters, displayed as <U+…>" ("folder" for the directory). It warns rather than refusing, since a
//!   command may hold a line break on purpose. What it says is decided for the bytes copied, so a change of the
//!   session's values during the confirmation changes neither the warning nor the clipboard.
//! - **A managed checkout is marked.** A session with a working copy shows the branch glyph after its age, named
//!   "Managed checkout of {repo}", with the old mark's whole explanation as hover help and description: where the
//!   checkout is, the session's own folder when it differs, and what happens to the checkout when its last session is
//!   deleted.
//! - **Keyboard order.** The bar is drawn after the terminal, so its two copy fields follow the top row and the
//!   terminal in Tab's order, as SPEC.md asks. Nothing else in the bar takes focus.
//!
//! What the bar shows is decided by [`Details::of`], a pure function of the session's row, the registry and the clock,
//! so tests read it without a window; [`bar`] only lays it out. The one thing it keeps is the copy fields'
//! confirmations ([`Copies`]), which the pane holds for its open session and drops with it, as the old view's keyed
//! remount dropped its own. The pane reads the row from the store by key, so the bar
//! names the open session also while a filter or the list's cap hides it from the list: the session query names the
//! open session, and the helm keeps it in the pushed collection then (`session_list::filters`, "The query names the
//! open session"). The bar never takes the title from the filtered listing. With no row in the store at all (before the
//! helm answers the query naming a newly opened session, say), the pane draws no bar.

use farhelm_helm_client::Collections;
use farhelm_proto::text::blank_placeholder;
use farhelm_proto::ui_channel::{HostView, SessionRow};
use gpui::{
    App, AppContext as _, Div, Entity, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement, Pixels, SharedString, StatefulInteractiveElement as _, Styled, div,
    prelude::FluentBuilder as _, px, relative, svg,
};

use crate::assets::Icon;
use crate::components::{CopyFeedback, CopyField, HoverHelp, Role, StatusMark};
use crate::environment_ui::ProviderUi;
use crate::host_text::{self, HostSentence, HostText, Line};
use crate::hosts::identity;
use crate::layers;
use crate::local_time::Zone;
use crate::session_list::abbreviate_home;
use crate::session_list::age::{self, Age};
use crate::session_list::environment::{Deleting, Environment};
use crate::session_list::row_marks::{self, HelpPlace, HostMark, Locality};
use crate::session_list::status::{self, EndedWords, Live};
use crate::theme::{self, Palette, metrics};

/// The bar's id, by which tests and the bridge find it. The ids of its parts start with it.
pub(crate) const BAR_ID: &str = "session-details";
/// The status badge's id.
pub(crate) const STATUS_ID: &str = "session-details-status";
/// The title's id.
pub(crate) const TITLE_ID: &str = "session-details-title";
/// The host mark's id: [`row_marks::host_mark`]'s for a "row" named [`BAR_ID`], which makes it. Named here for the
/// tests that find the mark.
#[cfg(test)]
pub(crate) const HOST_MARK_ID: &str = "session-details-host";
/// The host name's id.
pub(crate) const HOST_NAME_ID: &str = "session-details-host-name";
/// The age's id.
pub(crate) const AGE_ID: &str = "session-details-age";
/// The suspended environment's mark's id, in the age's place.
pub(crate) const SUSPENDED_ID: &str = "session-details-suspended";
/// The being-deleted environment's mark's id, in the age's place.
pub(crate) const DELETING_ID: &str = "session-details-deleting";
/// The managed-checkout mark's id.
pub(crate) const CHECKOUT_ID: &str = "session-details-checkout";
/// The directory's copy field's id.
pub(crate) const DIRECTORY_ID: &str = "session-details-directory";
/// The command line's copy field's id.
pub(crate) const COMMAND_ID: &str = "session-details-command";
/// The hidden-characters warning's id.
pub(crate) const WARNING_ID: &str = "session-details-copy-warning";

/// `.session-details`'s `min-height`, the same as the top row's. The bar never wraps and its content fits inside its
/// padding, so this is its height.
pub(crate) const MIN_HEIGHT: f32 = 30.;
/// `.session-details`'s padding: 3 pixels above and below, 10 at the sides.
const PAD_Y: f32 = 3.;
const PAD_X: f32 = 10.;
/// `.session-details`'s gap between its parts.
const GAP: f32 = 10.;
/// `.session-details-host`'s gap between the host mark and the name.
const HOST_GAP: f32 = 5.;
/// The branch glyph's side (`.managed-checkout-glyph`).
const CHECKOUT_GLYPH: f32 = 12.;
/// `.copy-warning`'s padding, vertical then horizontal, and its gap above the bar (`bottom: calc(100% + 4px)`).
const WARNING_PAD: (f32, f32) = (6., 8.);
const WARNING_GAP: f32 = 4.;

/// The advance of the bundled interface face's `0`, in ems: 1292 of Inter's 2048 units per em
/// (`assets/fonts/Inter-Regular.ttf`'s `hmtx` entry for the glyph). A CSS `ch` is this width.
const ZERO_ADVANCE_EM: f32 = 0.631;
/// The ended badge's horizontal padding on each side (`.session-details .status-badge`'s `padding: 1px 6px`).
const BADGE_PAD_X: f32 = 6.;

/// The ended badge's widest, padding included: the old `.status-badge`'s `max-width: 32ch` at its 12-pixel text, plus
/// its padding on both sides. The old sheet set no `box-sizing`, so its cap was the content box's, with the padding
/// outside it; GPUI's layout measures the border box, so the padding is added here. Wide enough that an ordinary exit
/// with a short annotation fits whole; a launch failure's detail ends in an ellipsis well before this.
fn badge_max_width() -> Pixels {
    metrics::TEXT_SM * (32. * ZERO_ADVANCE_EM) + px(2. * BADGE_PAD_X)
}

/// The hidden-characters warning's widest: `.copy-warning`'s `60ch` at its 12-pixel text. The old cap was the smaller of
/// that and 80% of the window's width; here the warning is held inside the bar's own width instead (see [`warning`]),
/// which is never wider than the window.
fn warning_max_width() -> Pixels {
    metrics::TEXT_SM * (60. * ZERO_ADVANCE_EM)
}

/// What the bar shows for one session, decided from its row, the registry and the viewer's clock. See the module
/// docs for the rules each field follows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Details {
    /// The status badge, or `None` for a stale or unclassified session.
    pub(crate) status: Option<Status>,
    pub(crate) title: HostText,
    /// The host, or `None` for a session that names no host.
    pub(crate) host: Option<Host>,
    /// The age, or `None` for a stale session, one with no stamp, or a viewer clock that cannot be used.
    pub(crate) age: Option<Age>,
    /// Whether the session's own environment is suspended, so the bar says so where the age would be, as the old bar
    /// did (`session_list::environment`).
    pub(crate) suspended: bool,
    /// What the bar says where the age would be while the session's own environment is being deleted, `None` otherwise
    /// (`session_list::environment::Environment::deleting`); the age is left out then.
    pub(crate) deleting: Option<Deleting>,
    /// The managed checkout, or `None` for a session without one.
    pub(crate) checkout: Option<Checkout>,
    /// The working directory's copy field.
    pub(crate) directory: CopyValue,
    /// The command line's copy field.
    pub(crate) command: CopyValue,
}

/// A copy field's two forms of one recorded value: what it draws and what a click copies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CopyValue {
    /// What the field draws: the directory with its home folded to `~` where that hides nothing
    /// ([`directory_label`]), or the command line as recorded.
    pub(crate) shown: HostText,
    /// The value as the session recorded it: what a click copies, and what the hover help and description name.
    pub(crate) raw: HostText,
    /// What the field draws and is named when `shown` would be blank ([`blank_placeholder`]).
    pub(crate) placeholder: Option<SharedString>,
}

/// A managed checkout as the bar's mark explains it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Checkout {
    /// The repository, `owner/name`.
    pub(crate) repo: HostText,
    /// The checkout folder's canonical path.
    pub(crate) path: HostText,
    /// The session's own folder, when it differs byte for byte from the checkout's path (a subdirectory, or the same
    /// folder spelled through a symlink), so the explanation does not let the checkout's path stand for it.
    pub(crate) folder: Option<HostText>,
    /// Whether the session runs in a disposable environment it owns, where Delete discards the checkout rather than
    /// moving it to the trash. The old app asked whether the session's host was session-owned and of a provider kind
    /// (`HostKind::environment_provider`); this asks whether it is session-owned and the helm describes its environment
    /// (`HostView::environment`), which the helm fills in only for a provider kind's host (from its sandbox's or
    /// container's binding; never for the local host or an SSH host), so no provider is named here, as AGENTS.md's
    /// provider rule asks. The two differ only for a provider host whose binding is missing, which reads as an
    /// ordinary checkout.
    pub(crate) in_environment: bool,
}

impl Checkout {
    /// The checkout a session uses now. The launcher and details bar both need the
    /// session's actual folder and the host's cleanup rule when explaining it.
    pub(crate) fn of(row: &SessionRow, collections: &Collections) -> Option<Self> {
        let copy = row.info.working_copy.as_ref()?;
        let cwd = row.info.cwd.as_str();
        Some(Self {
            repo: HostText::from_host(format!("{}/{}", copy.repo.owner, copy.repo.name)),
            path: HostText::from_host(copy.canonical_path.as_str()),
            folder: (cwd != copy.canonical_path).then(|| HostText::from_host(cwd)),
            in_environment: collections
                .get::<HostView>(&HostView::key(row.host))
                .is_some_and(|host| host.is_session_owned && host.environment.is_some()),
        })
    }
}

/// The badge: a live mark, or an ended status's words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    Live(Live),
    Ended(EndedWords),
}

impl Status {
    /// The pane's status badge for the row, including the live mark derived from its activity and seen state.
    /// An exit with no code reads `Exited`, and an unclassified row has no badge. Each surface decides when to show it.
    pub(super) fn of(row: &SessionRow) -> Option<Self> {
        status::live(row)
            .map(Self::Live)
            .or_else(|| status::ended_words(row, false).map(Self::Ended))
    }
}

/// Measure the row's effective activity with the viewer's clock and stamp it in `zone`. The row supplies its
/// creation time when it has no activity timestamp. A usable `now` is needed for either pane surface to show an age.
pub(super) fn activity_age(row: &SessionRow, now: Option<i64>, zone: Zone) -> Option<Age> {
    age::of(row.info.effective_activity(), now, zone)
}

/// The session's host as the bar names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Host {
    /// The row's host mark, `None` when the host cannot be placed.
    pub(crate) mark: Option<HostMark>,
    /// The provider of the session's own environment, whose mark the bar draws in place of the host mark, as the row
    /// does; `None` for an ordinary host.
    pub(crate) environment: Option<ProviderUi>,
    /// The helm's full name for the host, with the local host's own wording, or the environment's name ("dev
    /// sandbox") for a session in its own environment.
    pub(crate) name: HostText,
}

impl Details {
    /// What the bar shows for `row` over `collections`, with the viewer's clock at `now` (seconds since the epoch,
    /// [`age::seconds`]; `None` when it cannot be used, and no age is shown) and the age's stamp in `zone`.
    pub(crate) fn of(
        row: &SessionRow,
        collections: &Collections,
        now: Option<i64>,
        zone: Zone,
    ) -> Details {
        let status = if row.stale { None } else { Status::of(row) };
        let environment = Environment::of(row, collections);
        let host = (!row.host_name.is_empty()).then(|| {
            let mark = HostMark::of(row, collections, None);
            let local = mark.is_some_and(|mark| mark.locality == Locality::Local);
            Host {
                mark,
                environment: environment.map(|environment| environment.ui),
                name: environment.map_or_else(
                    || identity::named(&row.host_name, local),
                    |environment| HostText::from_host(environment.label()),
                ),
            }
        });
        let deleting = environment.and_then(Environment::deleting);
        let age = if row.stale || deleting.is_some() {
            None
        } else {
            activity_age(row, now, zone)
        };
        let cwd = row.info.cwd.as_str();
        Details {
            status,
            title: HostText::from_host(row.info.title.as_str()),
            host,
            age,
            suspended: environment.is_some_and(|environment| environment.suspended()),
            deleting,
            checkout: Checkout::of(row, collections),
            directory: CopyValue::new(directory_label(cwd), cwd),
            command: CopyValue::new(row.info.invocation.clone(), &row.info.invocation),
        }
    }

    /// Whether the badge pulses: a running session's mark. A stale session has no badge, so this never asks whether
    /// the host is connected.
    pub(crate) fn pulses(&self) -> bool {
        matches!(self.status, Some(Status::Live(live)) if live.pulses(false))
    }
}

/// `cwd` as the directory field shows it: a personal home folded to `~` as the session list folds it
/// ([`abbreviate_home`]), unless the fold would remove a presentation-unsafe character. Then the path is shown whole, so
/// abbreviating it cannot hide the escaped spelling of a character the copy holds (the old bar's `directory_label`).
pub(crate) fn directory_label(cwd: &str) -> String {
    let hidden = |text: &str| {
        text.chars()
            .filter(|ch| farhelm_proto::text::is_presentation_unsafe(*ch))
            .count()
    };
    let folded = abbreviate_home(cwd);
    if hidden(&folded) == hidden(cwd) {
        folded
    } else {
        cwd.to_owned()
    }
}

impl CopyValue {
    /// A field showing `shown` (or its marker, when it would show nothing) and copying `raw`.
    fn new(shown: String, raw: &str) -> Self {
        CopyValue {
            placeholder: blank_placeholder(&shown).map(SharedString::from),
            shown: HostText::from_host(shown),
            raw: HostText::from_host(raw),
        }
    }
}

/// The open session's copy fields' confirmations, the one state the bar keeps. The pane makes one with each terminal it
/// mounts and drops it with that terminal, so a confirmation never outlives its session's selection: choosing another
/// session and coming back starts with neither field confirming.
#[derive(Clone)]
pub(crate) struct Copies {
    directory: Entity<CopyFeedback>,
    command: Entity<CopyFeedback>,
}

impl Copies {
    /// Two fields at rest.
    pub(crate) fn new(cx: &mut App) -> Self {
        Copies {
            directory: cx.new(|_| CopyFeedback::default()),
            command: cx.new(|_| CopyFeedback::default()),
        }
    }

    /// The command field's confirmation, for a screenshot scene that shows one without a click.
    #[cfg(test)]
    pub(crate) fn command(&self) -> &Entity<CopyFeedback> {
        &self.command
    }

    /// The hidden-characters warning to show now, if any: for the directory's confirmed click first, as the old bar
    /// chose, then the command's. Read while drawing, so the bar redraws when either confirmation changes.
    pub(crate) fn warning(&self, cx: &App) -> Option<&'static str> {
        if self.directory.read(cx).copied_hidden() {
            Some("⚠ The copied folder contains hidden characters, displayed as <U+…>")
        } else if self.command.read(cx).copied_hidden() {
            Some("⚠ The copied command contains hidden characters, displayed as <U+…>")
        } else {
            None
        }
    }
}

/// The bar for `details`, its copy fields confirming as `copies` records, a running mark drawn at pulse step `pulse`
/// ([`crate::ticker::Pulsing::step`]). Drawn by the pane on every frame, uncached, like the top row; the screenshot
/// goldens draw it on its own.
///
/// The bar itself clips what does not fit, so it is wrapped in a box that does not: the hidden-characters warning hangs
/// above the bar from that box, over the bottom of the terminal, where the bar's clip cannot reach it.
pub(crate) fn bar(details: Details, copies: &Copies, pulse: u64, cx: &App) -> Div {
    let palette = theme::palette(cx);
    let row = div()
        .id(BAR_ID)
        .debug_selector(|| BAR_ID.into())
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(px(GAP))
        .min_h(px(MIN_HEIGHT))
        .px(px(PAD_X))
        .py(px(PAD_Y))
        .overflow_hidden()
        .whitespace_nowrap()
        .bg(theme::color(palette.bg_1))
        .text_size(metrics::TEXT_MD)
        .text_color(theme::color(palette.fg_0))
        .children(
            details
                .status
                .map(|status| badge(status, STATUS_ID, false, pulse, palette)),
        )
        .child(title(&details.title, palette))
        .children(details.host.map(|host| host_part(host, palette)))
        .children(details.age.map(|age| age_part(age, AGE_ID)))
        // A suspended environment says so where the age would be (left out while stale anyway), as the old bar did.
        .children(details.suspended.then(|| {
            row_marks::suspended_mark(SUSPENDED_ID.into(), true, palette.fg_1, HelpPlace::Ordinary)
        }))
        // So does an environment being deleted, with why the session's actions are unavailable.
        .children(details.deleting.map(|deleting| {
            row_marks::deleting_mark(
                DELETING_ID.into(),
                deleting,
                palette.fg_1,
                HelpPlace::Ordinary,
            )
        }))
        .children(
            details
                .checkout
                .map(|checkout| checkout_mark(CHECKOUT_ID, checkout, palette)),
        )
        .child(copy_fields(details.directory, details.command, copies));
    div()
        .relative()
        .flex_none()
        .child(row)
        .children(copies.warning(cx).map(|text| warning(text, palette)))
}

/// The managed-checkout mark: the branch glyph in the quiet ink, which never shrinks, named for the repository, with the
/// old mark's explanation as its hover help and description (`ManagedCheckoutMark` in `icons.rs` at the reference
/// commit, which said each host value escaped and kept the session's own folder in view when it was not the checkout's).
pub(crate) fn checkout_mark(id: &'static str, checkout: Checkout, palette: &Palette) -> Div {
    let name = HostSentence::new(id)
        .text("Managed checkout of ")
        .host(&checkout.repo)
        .accessible_name();
    let mut help = HostSentence::new(format!("{id}-help"))
        .text("Managed checkout of ")
        .host(&checkout.repo)
        .text(" at ")
        .host(&checkout.path)
        .text(".");
    if let Some(folder) = &checkout.folder {
        help = help.text(" Session folder: ").host(folder).text(".");
    }
    help = help.text(if checkout.in_environment {
        " Delete discards this checkout with its session, without moving it to the trash, unless another session still \
         uses it. Deleting the environment's last session, or the whole environment, permanently removes every checkout \
         and unpushed change in it."
    } else {
        " Farhelm made the checkout folder for its first session. When the last session using it is deleted, the \
         checkout moves to the trash."
    });
    div().flex_none().child(
        StatusMark::new(id, name, HoverHelp::sentence(help), Role::Label, "").content(
            div().flex().items_center().child(
                svg()
                    .path(Icon::Branch.path())
                    .size(px(CHECKOUT_GLYPH))
                    .text_color(theme::color(palette.fg_1)),
            ),
        ),
    )
}

/// The two copy fields in one box that starts from nothing and grows into whatever width the bar's other parts leave
/// (`flex: 1 1 0`, with no minimum), so the fields are what gives way first and the title and host keep their width
/// while the fields have any (SPEC.md). Inside the box each field still takes its value's width and the two shrink as
/// the old fields did (three times an ordinary item's), which only decides how the box's width is split between them.
/// The box adds no width the bar has to find, since it asks for none, and with room to spare it fills the rest of the
/// bar while the fields stay at their values' width at its left, as the old fields sat after the age.
fn copy_fields(directory: CopyValue, command: CopyValue, copies: &Copies) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .flex_1()
        .min_w_0()
        .gap(px(GAP))
        .child(copy_field(DIRECTORY_ID, directory, &copies.directory))
        .child(copy_field(COMMAND_ID, command, &copies.command))
}

/// One of the bar's copy fields under `id`, for `value`, its confirmation kept in `feedback`.
fn copy_field(id: &'static str, value: CopyValue, feedback: &Entity<CopyFeedback>) -> CopyField {
    // A placeholder names the field in place of this label (`CopyField::placeholder`).
    let label = HostSentence::new(id).host(&value.shown).accessible_name();
    let help = HostSentence::new(format!("{id}-help"))
        .host(&value.raw)
        .text(" — click to copy");
    CopyField::new(
        id,
        label,
        HoverHelp::sentence(help),
        &value.shown,
        value.raw.raw().to_owned(),
        feedback,
    )
    .placeholder(value.placeholder)
}

/// The hidden-characters warning, opening upward from the bar 10 pixels in from its left edge, as the old
/// `.copy-warning` did: the raised ground, the bright ink at 12 pixels, the menu shadow, wrapping rather than clipping.
/// It is held inside the bar's width (10 pixels clear of either edge) where the old one was capped at 80% of the window
/// (a recorded deviation), so it stays inside the pane and the window however long it is. It takes no input: it has no
/// listener, so GPUI gives it no hitbox, and the terminal under it keeps every press.
fn warning(text: &'static str, palette: &Palette) -> Div {
    div()
        .absolute()
        .left(px(PAD_X))
        .right(px(PAD_X))
        .bottom(relative(1.))
        .pb(px(WARNING_GAP))
        .flex()
        .flex_row()
        .child(
            div()
                .id(WARNING_ID)
                .debug_selector(|| WARNING_ID.into())
                .role(Role::Status)
                .aria_label(text)
                .min_w_0()
                .max_w(warning_max_width())
                .py(px(WARNING_PAD.0))
                .px(px(WARNING_PAD.1))
                .bg(theme::color(palette.bg_2))
                .text_color(theme::color(palette.fg_bright))
                .text_size(metrics::TEXT_SM)
                .rounded(metrics::RADIUS_MD)
                .shadow(layers::menu_shadow(palette))
                .child(text),
        )
}

/// The status badge. A live status is the row's mark alone, which never shrinks, named by its word and helped by it
/// (the old dot's tooltip was the badge's text). An ended status is its words in the status's color, which may shrink
/// to an ellipsis, with the whole wording as hover help and name (`.session-details .status-badge`).
///
/// Drawn under `id`, its help and words keyed from it. `stale` draws a live mark as a stale row's, grey and still, for
/// the unavailable card's band (`unavailable.rs`), which shows a stale session's last-known status with this badge;
/// the bar itself never shows a stale session's status, and passes `false`.
pub(crate) fn badge(
    status: Status,
    id: &'static str,
    stale: bool,
    pulse: u64,
    palette: &Palette,
) -> Div {
    match status {
        Status::Live(live) => div().flex_none().child(
            StatusMark::new(
                id,
                live.word(),
                HoverHelp::tooltip(live.word()),
                Role::Label,
                live.word(),
            )
            .content(
                div()
                    .flex()
                    .items_center()
                    .child(live.mark(palette, false, stale, pulse)),
            )
            .described(false),
        ),
        Status::Ended(ended) => {
            let label = ended.line(id.into()).accessible_text();
            let help = match ended.line(format!("{id}-help").into()) {
                Line::App(words) => HoverHelp::tooltip(words),
                Line::Sentence(sentence) => HoverHelp::sentence(sentence),
            };
            let quotes_host = ended.host.is_some();
            // The words and the host value side by side on one line, each its own text element (a host value drawn
            // alone keeps its direction to itself), so each can be cut short on its own, which a sentence's one text
            // element cannot do. With a host value it is the part that gives way, since the words carry the status
            // itself and the exit code; with none the words do. The old badge was one text run that ellipsized at its
            // end; here, once the host value is cut to nothing, the words never shrink, so a bar too narrow for them
            // clips them at the badge's edge with no ellipsis (and perhaps a dangling dash). A shrink factor far below
            // the value's would not help: any shrink at all ellipsizes the words while the value still shows. That
            // takes a pane far narrower than the 320-pixel floor leaves for the badge beside a title and a host, and
            // the whole wording stays the hover help.
            let wording = div()
                .flex()
                .flex_row()
                .min_w_0()
                .overflow_hidden()
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .when(quotes_host, |words| words.flex_none())
                        .child(ended.words),
                )
                .children(
                    ended
                        .host
                        .map(|host| host_text::element(&host).min_w_0().truncate()),
                );
            div()
                .min_w_0()
                .max_w(badge_max_width())
                .px(px(BADGE_PAD_X))
                .py(px(1.))
                .overflow_hidden()
                .child(
                    StatusMark::new(id, label, help, Role::Label, "")
                        .ink(ended.kind.color(palette, false))
                        .truncate()
                        .content(wording)
                        .described(false),
                )
        }
    }
}

/// The title, semibold, cut with an ellipsis when the bar runs out of room, with the whole title as hover help through
/// the host-text element (`.session-details .title`).
fn title(title: &HostText, palette: &Palette) -> impl IntoElement {
    StatusMark::new(
        TITLE_ID,
        HostSentence::new(TITLE_ID).host(title).accessible_name(),
        HoverHelp::sentence(HostSentence::new(format!("{TITLE_ID}-help")).host(title)),
        Role::Label,
        "",
    )
    .ink(palette.fg_0)
    .truncate()
    .content(value(title))
    .described(false)
}

/// The host: its mark, which never shrinks, then its name in the bright foreground, semibold, cut with an ellipsis when
/// the bar runs out of room, with the shown name as hover help (`.session-details-host`).
fn host_part(host: Host, palette: &Palette) -> Div {
    let name = &host.name;
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(HOST_GAP))
        .min_w_0()
        .overflow_hidden()
        .children(
            match host.environment {
                Some(ui) => Some(
                    row_marks::environment_mark(BAR_ID, ui, palette.fg_1, HelpPlace::Ordinary)
                        .into_any_element(),
                ),
                None => host.mark.map(|mark| {
                    row_marks::host_mark(BAR_ID, mark, palette, HelpPlace::Ordinary)
                        .into_any_element()
                }),
            }
            .map(|mark| div().flex_none().flex().items_center().child(mark)),
        )
        .child(
            StatusMark::new(
                HOST_NAME_ID,
                HostSentence::new(HOST_NAME_ID).host(name).accessible_name(),
                HoverHelp::sentence(
                    HostSentence::new(SharedString::from(format!("{HOST_NAME_ID}-help")))
                        .host(name),
                ),
                Role::Label,
                "",
            )
            .ink(palette.fg_bright)
            .truncate()
            .content(value(name))
            .described(false),
        )
}

/// A host value at the bar's text size, semibold, on one line ending in an ellipsis when it does not fit: the title and
/// the host name share this look.
fn value(text: &HostText) -> Div {
    host_text::element(text)
        .min_w_0()
        .truncate()
        .text_size(metrics::TEXT_MD)
        .font_weight(FontWeight::SEMIBOLD)
}

/// The age: the row's relative age in the quiet foreground and the monospace face, which never shrinks, named and
/// helped by the full stamp (`.status-time`). Drawn under `id`: the bar's, or the unavailable card's band's, which
/// shows a stale session's age in the bar's place.
pub(crate) fn age_part(age: Age, id: &'static str) -> Div {
    div().flex_none().child(
        StatusMark::new(
            id,
            age.full.clone(),
            HoverHelp::tooltip(age.full),
            Role::Label,
            age.short,
        )
        .monospace()
        .described(false),
    )
}
