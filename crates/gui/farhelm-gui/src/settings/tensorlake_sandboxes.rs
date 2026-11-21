//! Settings → Tensorlake's **Sandboxes not on this helm**: the sandboxes with Farhelm's reserved name prefixes that no
//! host, recipe build or preparation on this helm owns, each with a confirmed Delete that terminates it and deletes its
//! snapshots.
//!
//! SPEC.md, "Topology" (the paragraph that starts "Settings also lists **Sandboxes not on this helm**"); SPEC_impl.md,
//! "The UI channel" (the Tensorlake settings routes' bound) and "Tensorlake account". Ported from the old app's
//! `OrphanList` (`tensorlake/orphans.rs`) and its look from the `.tensorlake-orphans*` rules in `app.css`, all at
//! reference commit `24bc0580`, with the old words. Here's the TLDR of its rules:
//!
//! - **Shown with a saved key.** The list needs the saved API key, so it reads nothing and is not drawn while the
//!   opening's account says no key is saved, unless something is left to show (rows, a failure, an open question).
//!   With a key it stays drawn even when empty, with its heading and Refresh (SPEC.md).
//! - **Read when it can change, never on a timer.** The list is not pushed, and reading it lists the user's whole
//!   Tensorlake account, so it is read only when its answer can have changed: when the account read first says a key is
//!   saved (the opening), on Refresh, after a Save or Clear of the key in either form of this opening (a Clear empties
//!   it at once), after every answer to a Delete, and when the pushed hosts list gains or loses a Tensorlake host, which
//!   changes what this helm owns. It is never read again on other changes of the fleet; the old list re-read on every
//!   fleet notice while healthy (decided by the migration's coordinator on 2026-11-15).
//! - **Rounds.** Each read starts a new round, and only the newest round's answer is shown: two reads can be out at once
//!   (a Tensorlake host coming while the first read is out), and the UI channel answers them in order, so without this
//!   the older answer would end "Checking…" and show its rows for a moment before the newer one replaced them. A Clear
//!   also ends the round, so nothing a read still out answers is shown once the key is gone.
//! - **What a read shows.** "Checking…" while the current round's read is out; then the rows, or "None found · checked
//!   <time>" when there are none. A failed read shows the helm's reason and keeps the rows it had, rather than claiming
//!   the account has none (SPEC.md). The helm stops listing at its host reply bound and answers like any other
//!   unanswered Tensorlake call.
//! - **Delete always asks**, inline under the list, naming the sandbox, with focus on Cancel since it cannot be undone,
//!   and the answer acts on the row as it was asked about. While a Delete is out, Refresh and every Delete are
//!   unavailable. A row whose cleanup failed offers Retry cleanup, which asks the same question; a running cleanup shows
//!   Cleaning up…. Both come from the helm's list, so they survive closing Settings. A refused Delete shows the helm's
//!   words under the list, and the helm's answer at its bound (the cleanup carries on) is such a refusal; either way the
//!   list is read again, which shows how the cleanup stands.
//! - **Plain commands.** Nothing here is a saved choice, so reads go out with [`crate::operations::read`] and Deletes
//!   with [`crate::operations::send`]. Every name, state and reason is the helm's or Tensorlake's, drawn as host text.
//! - **Times are the viewer's.** "None found · checked HH:MM" is the hours and minutes of the last answered read in the
//!   viewer's local time, as SPEC.md asks and the old list showed it; a row's creation time is the app's stamp, to the
//!   second, in local time too ([`crate::local_time`]). The old creation time was UTC, to the minute; the maintainer
//!   decided on 2026-11-15 that the app's times are local. The read's instant is kept, not its words, so a zone change
//!   shows at the next draw; a clock at or before 1970 is broken, and "None found" then gives no time at all.
//!
//! NOTE: A successful Test no longer reads a failed list again, as the old list did after a rejected key was fixed
//! elsewhere; Refresh does (the coordinator's list of re-reads above, 2026-11-15).

use farhelm_helm_client::{Collections, Method};
use farhelm_proto::tensorlake::{OrphanCleanup, OrphanSandbox};
use farhelm_proto::ui_channel::{Availability, Collection, HostView};
use gpui::{
    App, Context, Div, Entity, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    Role, SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div,
    px,
};

use super::card;
use super::choices;
use super::tensorlake_key::{TensorlakeAccount, path};
use crate::components::{Button, HoverHelp, Tier};
use crate::focus::Mark;
use crate::host_text::{self, HostSentence, HostText};
use crate::hosts::offers::HostKind;
use crate::local_time::{self, Zone};
use crate::prompt::{self, Answer, Inline, Question};
use crate::session::{self, ObservingCurrent};
use crate::session_list::age;
use crate::theme::{self, metrics};
use crate::ticker;

/// The list's element id; its parts are `-refresh`, `-status`, `-error` and `-delete-error` after it, and row `<n>`'s
/// `tensorlake-sandbox-<n>`, with `-delete` and `-cleanup-error` after that.
pub(super) const ID: &str = "tensorlake-sandboxes";
const STATUS_ID: &str = "tensorlake-sandboxes-status";
const ERROR_ID: &str = "tensorlake-sandboxes-error";
const DELETE_ERROR_ID: &str = "tensorlake-sandboxes-delete-error";

/// The old words.
pub(super) const HEADING: &str = "Sandboxes not on this helm";
pub(super) const HELP: &str = "These Farhelm sandboxes have no host or snapshot build on this helm. Nothing is deleted automatically.";

/// Keep a sandbox's identity, state and cleanup failure together inside its row.
const ROW_GAP: f32 = 4.;

/// What the list knows of the account that decides whether and when it reads: whether a key is saved, and how many
/// key saves this opening has had. A change of either reads the list again (the module docs).
type KeyState = (bool, u64);

/// The pushed fleet's Tensorlake hosts, by id in the helm's order: a change reads the list again (the module docs).
type Sandboxed = Vec<i64>;

/// The list, a view of its own that the Tensorlake section makes with itself and keeps for the opening.
pub(super) struct Sandboxes {
    account: Entity<TensorlakeAccount>,
    /// The rows as the last answered read listed them.
    rows: Vec<OrphanSandbox>,
    /// Why the last read failed, until a read answers; the rows from before stay.
    failure: Option<HostText>,
    /// Whether the current round's read is out.
    checking: bool,
    /// When the last read that answered found the rows above: `None` before one answered, then the instant in seconds
    /// since the epoch, worded as it is drawn, or `None` inside from a broken clock, which shows no time.
    checked_at: Option<Option<i64>>,
    /// The current round: one more for every read started. An answer of another round is dropped.
    round: u64,
    /// The account state the list last acted on; `None` until the account read answers.
    key: Option<KeyState>,
    /// The fleet's Tensorlake hosts as last seen; `None` until the hosts collection is first read whole.
    sandboxed: Option<Sandboxed>,
    /// The open Delete question, drawn under the rows until it is answered.
    question: Option<Inline<OrphanSandbox>>,
    /// The sandbox the last question asked about, whose failed cleanup's line the question replaces while it is open.
    asked: Option<String>,
    /// Whether a Delete is out.
    deleting: bool,
    /// The helm's words for the last refused Delete, until the next Delete is asked.
    delete_failure: Option<HostText>,
    _account: Subscription,
    _fleet: ObservingCurrent,
}

impl Sandboxes {
    /// The list over `account`, the opening's. It reads once the account says a key is saved.
    pub(super) fn new(account: Entity<TensorlakeAccount>, cx: &mut Context<Self>) -> Self {
        let observing = cx.observe(&account, |list: &mut Self, _, cx| list.follow_account(cx));
        let mut list = Sandboxes {
            account,
            rows: Vec::new(),
            failure: None,
            checking: false,
            checked_at: None,
            round: 0,
            key: None,
            sandboxed: None,
            question: None,
            asked: None,
            deleting: false,
            delete_failure: None,
            _account: observing,
            _fleet: session::observe_current(cx, |list: &mut Self, cx| list.follow_fleet(cx)),
        };
        // The fleet as it stands now is the memo a later change is told apart from, so the first push after opening
        // that adds a Tensorlake host still reads.
        list.sandboxed =
            session::current_store(cx).and_then(|store| sandboxed(store.read(cx).collections()));
        // The account may have answered already, when another section read it first.
        list.follow_account(cx);
        list
    }

    /// The account changed: read again if whether a key is saved, or which key it is, changed. A key cleared empties the
    /// list at once and drops any read still out.
    fn follow_account(&mut self, cx: &mut Context<Self>) {
        let account = self.account.read(cx);
        let Some(settings) = account.settings() else {
            return;
        };
        let now = (settings.api_key_set, account.key_saves());
        if self.key == Some(now) {
            return;
        }
        self.key = Some(now);
        if now.0 {
            self.read(cx);
        } else {
            self.round += 1;
            self.rows.clear();
            self.failure = None;
            self.checking = false;
            self.checked_at = None;
        }
        cx.notify();
    }

    /// The store changed: read again if the fleet gained or lost a Tensorlake host. The first fleet seen only becomes
    /// the memo, and a hosts collection that is not available proves nothing.
    fn follow_fleet(&mut self, cx: &mut Context<Self>) {
        let Some(store) = session::current_store(cx) else {
            return;
        };
        let Some(now) = sandboxed(store.read(cx).collections()) else {
            return;
        };
        let changed = self.sandboxed.as_ref().is_some_and(|before| *before != now);
        self.sandboxed = Some(now);
        if changed && self.key_saved() {
            self.read(cx);
            cx.notify();
        }
    }

    /// Whether the account says a key is saved.
    fn key_saved(&self) -> bool {
        self.key.is_some_and(|(saved, _)| saved)
    }

    /// Start a new round and read the list, the answer going to this view if it is still open and still on that round.
    fn read(&mut self, cx: &mut Context<Self>) {
        self.round += 1;
        self.checking = true;
        let round = self.round;
        let list = cx.weak_entity();
        choices::read_route(
            path(&["orphans"]),
            "GET /api/tensorlake/orphans",
            cx,
            move |answer: Result<Vec<OrphanSandbox>, HostText>, cx| {
                let _ = list.update(cx, |list, cx| {
                    if list.round == round {
                        list.answered(answer, cx);
                    }
                });
            },
        );
    }

    /// The current round's read answered (the module docs).
    fn answered(&mut self, answer: Result<Vec<OrphanSandbox>, HostText>, cx: &mut Context<Self>) {
        self.checking = false;
        match answer {
            Ok(rows) => {
                self.rows = rows;
                self.failure = None;
                self.checked_at = Some(age::seconds(ticker::wall_now(cx)));
            }
            Err(reason) => self.failure = Some(reason),
        }
        cx.notify();
    }

    /// Refresh: read again. Does nothing while a read or a Delete is out, when Refresh is busy. Refresh is drawn only
    /// with a key saved, so that needs no check here.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.checking || self.deleting {
            return;
        }
        self.read(cx);
        cx.notify();
    }

    /// The user pressed `row`'s Delete or Retry cleanup at `mark`: ask, inline, whether to delete it.
    fn ask(
        &mut self,
        row: OrphanSandbox,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.deleting || row.cleanup == Some(OrphanCleanup::Running) {
            return;
        }
        self.delete_failure = None;
        self.asked = Some(row.id.clone());
        let name = HostText::from_host(row.name.clone());
        let list = cx.weak_entity();
        let question = Question::new(
            HostSentence::new(SharedString::from(format!("{ID}-question")))
                .text("Delete sandbox ")
                .host(&name)
                .text("?"),
            Answer {
                label: "Delete sandbox".into(),
                help: HoverHelp::tooltip(
                    "Terminate this sandbox and delete its files, sessions and snapshots",
                ),
                tier: Tier::Danger,
            },
            row,
        )
        .line("Its files, sessions and snapshots will be deleted.")
        .cancel_answer(Answer {
            label: "Cancel".into(),
            help: HoverHelp::tooltip("Cancel: keep this sandbox"),
            tier: Tier::Neutral,
        })
        // Deleting a sandbox cannot be undone (SPEC.md, "Opening and closing": focus starts on Cancel then).
        .starts_on_cancel();
        self.question = Some(prompt::ask_inline(
            question,
            mark,
            move |row, _, cx| {
                let _ = list.update(cx, |list, cx| list.delete(row, cx));
            },
            window,
            cx,
        ));
        cx.notify();
    }

    /// The user confirmed deleting `row`: send its Delete, and read the list again whatever it answers.
    fn delete(&mut self, row: OrphanSandbox, cx: &mut Context<Self>) {
        self.asked = None;
        if self.deleting {
            return;
        }
        let path = match path(&["orphans", &row.id]) {
            Ok(path) => path,
            Err(error) => {
                self.delete_failure = Some(HostText::from_host(error.to_string()));
                cx.notify();
                return;
            }
        };
        let list = cx.weak_entity();
        let sent = crate::operations::send(
            Method::Delete,
            path,
            None,
            "DELETE /api/tensorlake/orphans/[sandbox]",
            cx,
            move |answer, cx| {
                let _ = list.update(cx, |list, cx| {
                    list.deleting = false;
                    if let Err(reason) = answer {
                        list.delete_failure = Some(reason);
                    }
                    if list.key_saved() {
                        list.read(cx);
                    }
                    cx.notify();
                });
            },
        );
        self.deleting = sent;
        cx.notify();
    }

    /// The sandbox the question asks about while it is open.
    fn asking_about(&self, cx: &App) -> Option<&String> {
        self.question
            .as_ref()
            .filter(|question| question.is_open(cx))
            .and(self.asked.as_ref())
    }

    /// Whether the list is drawn: with a saved key, or while it still has something to show (the module docs).
    fn drawn(&self, cx: &App) -> bool {
        self.key_saved()
            || !self.rows.is_empty()
            || self.failure.is_some()
            || self.asking_about(cx).is_some()
            || self.delete_failure.is_some()
    }

    /// One row: the sandbox's name, its state with "from another helm" and its creation time (in `zone`, the viewer's),
    /// a failed cleanup's reason, and Delete, Retry cleanup or Cleaning up….
    fn row(&self, row: &OrphanSandbox, index: usize, zone: Zone, cx: &Context<Self>) -> Div {
        let palette = theme::palette(cx);
        let id = format!("tensorlake-sandbox-{index}");
        let mut details = HostSentence::new(SharedString::from(format!("{id}-details")))
            .host(&HostText::from_host(row.state.clone()));
        if row.from_another_helm {
            details = details.text(" · from another helm");
        }
        if let Some(created) = row.created_at {
            let created = local_time::stamp(created / 1000, zone);
            details = details.text(format!(" · Created {created}"));
        }
        let cleanup_failure = match &row.cleanup {
            Some(OrphanCleanup::Failed { reason }) if self.asking_about(cx) != Some(&row.id) => {
                let line = format!("{id}-cleanup-error");
                let sentence = HostSentence::new(SharedString::from(line.clone()))
                    .text("Cleanup unfinished: ")
                    .host(&HostText::from_host(reason.clone()));
                Some(
                    choices::outcome_line(line, Role::Alert, theme::color(palette.danger))
                        .aria_label(sentence.accessible_name())
                        .child(sentence),
                )
            }
            _ => None,
        };
        let (label, help) = match row.cleanup {
            Some(OrphanCleanup::Running) => (
                "Cleaning up…",
                "Its cleanup is running; Refresh shows when it ends",
            ),
            Some(OrphanCleanup::Failed { .. }) => (
                "Retry cleanup",
                "Try this sandbox's cleanup again, after asking",
            ),
            None => (
                "Delete",
                "Delete this sandbox and its snapshots, after asking",
            ),
        };
        let weak = cx.weak_entity();
        let asked = row.clone();
        let delete = Button::new(
            format!("{id}-delete"),
            label,
            HoverHelp::tooltip(help),
            Role::Button,
        )
        .tier(Tier::Neutral)
        // Busy, not disabled, while a Delete is out, so the Delete that was confirmed gets focus back from the question
        // and keeps it (SPEC_impl.md, "Drafts"); only a running cleanup offers nothing to press.
        .busy(self.deleting)
        .disabled(row.cleanup == Some(OrphanCleanup::Running))
        .on_click(move |_, window, cx| {
            let mark = Mark::take(cx);
            let asked = asked.clone();
            let _ = weak.update(cx, |list, cx| list.ask(asked, &mark, window, cx));
        });
        div()
            .debug_selector(move || id.clone())
            .flex()
            .items_center()
            .gap(px(10.))
            .min_w_0()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(ROW_GAP))
                    .child(
                        div()
                            .text_size(metrics::TEXT_MD)
                            .text_color(theme::color(palette.fg_0))
                            .child(host_text::element(&HostText::from_host(row.name.clone()))),
                    )
                    .child(
                        div()
                            .text_size(metrics::TEXT_SM)
                            .text_color(theme::color(palette.fg_1))
                            .child(details),
                    )
                    .children(cleanup_failure),
            )
            .child(div().flex_none().child(delete))
    }

    /// A status line: "Checking…" or "None found · checked <time>".
    fn status(text: SharedString, cx: &App) -> gpui::Stateful<Div> {
        choices::outcome_line(
            STATUS_ID,
            Role::Status,
            theme::color(theme::palette(cx).fg_1),
        )
        .aria_label(text.clone())
        .child(text)
    }
}

impl Render for Sandboxes {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.drawn(cx) {
            // Hidden rather than empty: an empty child would still take a gap in the section's column.
            return div().id(ID).hidden();
        }
        let weak = cx.weak_entity();
        let refresh = self.key_saved().then(|| {
            Button::new(
                format!("{ID}-refresh"),
                "Refresh",
                HoverHelp::tooltip(
                    "Refresh: check Tensorlake again for sandboxes not on this helm",
                ),
                Role::Button,
            )
            .tier(Tier::Neutral)
            .compact()
            // Busy, not disabled, so a Refresh pressed by keyboard keeps focus while its read is out.
            .busy(self.deleting || self.checking)
            .on_click(move |_, _, cx| {
                let _ = weak.update(cx, |list, cx| list.refresh(cx));
            })
        });
        let heading = card::heading(
            HEADING,
            None,
            refresh.map(IntoElement::into_any_element),
            cx,
        )
        .id(SharedString::from(format!("{ID}-heading")))
        .role(Role::Heading)
        .aria_label(HEADING);
        let zone = local_time::zone(cx);
        let status = if self.checking {
            Some(Self::status("Checking…".into(), cx))
        } else if self.rows.is_empty() && self.failure.is_none() {
            self.checked_at.map(|checked| {
                let words = match checked {
                    Some(at) => format!("None found · checked {}", local_time::clock(at, zone)),
                    None => "None found".to_owned(),
                };
                Self::status(words.into(), cx)
            })
        } else {
            None
        };
        let rows = status.into_iter().map(IntoElement::into_any_element).chain(
            self.rows
                .iter()
                .enumerate()
                .map(|(index, row)| self.row(row, index, zone, cx).into_any_element()),
        );
        let failure = |id: &'static str, reason: &HostText| {
            choices::host_failure_line(id, HostSentence::new(id).host(reason), cx)
        };
        div()
            .id(ID)
            .role(Role::Region)
            .aria_label(HEADING)
            .flex()
            .flex_col()
            .gap(px(8.))
            .w_full()
            .min_w_0()
            .child(heading)
            .child(card::rows(
                rows.chain(
                    self.question
                        .as_ref()
                        .filter(|question| question.is_open(cx))
                        .cloned()
                        .map(IntoElement::into_any_element),
                )
                .chain(
                    self.failure
                        .as_ref()
                        .map(|reason| failure(ERROR_ID, reason).into_any_element()),
                )
                .chain(
                    self.delete_failure
                        .as_ref()
                        .map(|reason| failure(DELETE_ERROR_ID, reason).into_any_element()),
                ),
                cx,
            ))
            .child(choices::help(HELP, cx))
    }
}

/// The fleet's Tensorlake hosts ([`Sandboxed`]), or `None` while the hosts collection is missing or not available,
/// when what it lists proves nothing about what this helm owns.
fn sandboxed(collections: &Collections) -> Option<Sandboxed> {
    let meta = collections.meta(Collection::Hosts)?;
    if meta.availability != Availability::Available {
        return None;
    }
    Some(
        collections
            .ordered::<HostView>()
            .into_iter()
            .filter(|host| HostKind::of(host).is_a_tensorlake_sandbox())
            .map(|host| host.id)
            .collect(),
    )
}
