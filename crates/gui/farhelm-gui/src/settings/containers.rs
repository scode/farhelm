//! Settings → Containers: whether each container host can run containers, as the helm's Podman checks say, and
//! **Containers not on this helm**, the containers this helm owns that no completed session on it holds, each with a
//! confirmed Delete.
//!
//! SPEC.md, "Container sessions" ("Settings → Containers shows probe status per host and a refreshable **Containers not
//! on this helm** list with confirmed Delete and retryable failure"); SPEC_impl.md, "Podman places, creation and
//! images". Ported from the old app's `ContainersSettings` (`containers_settings.rs`, with the routes in
//! `api/environments.rs`) and its look from the `.host-settings-section` and `.recipe-row` rules in `app.css`, all at
//! reference commit `24bc0580`. Here's the TLDR of its rules:
//!
//! - **The hosts are the pushed hosts with a Podman check.** The hosts collection carries each container host's check
//!   (`HostView::container_check`), and carries one for exactly the hosts that can hold containers (the helm's
//!   `can_host_environments`). A host is listed when it can hold containers by the GUI's own rule
//!   (`HostKind::holds_containers`, which the launcher picks container hosts by too) and carries a check, as Settings →
//!   Recipes' places use them too. Each row is named by the helm's name for the host, in the registry's order, and
//!   reads Ready, the check's reason, or "this host needs a Podman check" before any check of the host's current
//!   address has passed or failed, with "· Refreshing…" while one runs: the words the helm's `GET /api/environment-places` (since removed) gave the old
//!   section for the same check. Everything here is drawn from the push, so a check's result, a host added or removed,
//!   a rename or a retarget shows the moment it is pushed, with nothing read again.
//! - **Checked when first shown, on Refresh, and when a host needs it.** As the old section did on opening and on
//!   Refresh, a round asks the helm to check every container host again in the background ([`check_container_hosts`]);
//!   the results arrive through the push. The helm checks a host on its own only when a container launch or reconnect
//!   needs it, and the old section's re-read on every fleet notice is what checked a host added or retargeted while it
//!   was open, so the section follows the store and asks, once per round, for a check of each container host whose
//!   current address has no result and no check running (`Containers::asked`). A host moved back to an address asked
//!   about earlier in the round is asked again: only the current addresses are remembered. A round that starts before the hosts list arrives checks every host once it does.
//!   Nothing is asked while the hosts list is unavailable: what it still lists may be stale.
//! - **A refused check shows until the host's next result.** A check the helm refuses replaces the hosts with its
//!   reason, as the old places read's refusal did until its next read; here that lasts until the push brings a newer
//!   result for that host, the host leaves the pushed list, or the next Refresh. The usual refusal is of a host removed
//!   between the push and the request, so its "not found" must go with the host rather than wait for Refresh.
//! - **The leftovers are read when first shown, on Refresh and after a Delete.** They are not pushed, so the section
//!   reads them (the pattern Agent trust follows), and a reply reaches only the section it was sent for. Each Refresh
//!   starts a new round, and the answers of an older round are dropped (`Containers::round`).
//! - **Delete always asks.** Delete asks inline, under the lists, naming the container, with focus on Cancel since the
//!   removal cannot be undone; its answer acts on the row as it was asked about. A leftover with a host row goes through
//!   ordinary host removal (`DELETE /api/hosts/{id}`), whose connection hold and provisioning lock it needs; one without
//!   goes to `DELETE /api/container-orphans` with its place and canonical id, which the helm checks for ownership again
//!   before removing anything. While a Delete is out, Refresh and every Delete are unavailable, as in the old section. A
//!   done Delete reads the leftovers again; a refusal, or the helm's answer that the host did not answer within its
//!   bound, shows under the section with the helm's words and leaves the list as it was, so Delete can be tried again.
//! - **Plain commands.** None of this saves a choice the dialog shows, so nothing goes through the operation store:
//!   reads go out with [`crate::operations::read`], and the checks and Deletes with [`crate::operations::send`], their
//!   answers handled here. Every name, reason and refusal is the helm's or a host's, drawn as host text.
//!
//! NOTE: The rows show as soon as the section does, before the checks it asks for, where the old section showed nothing
//! new until its checks were asked for and its second read answered.

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::environment::ContainerLeftoverView;
use farhelm_proto::ui_channel::{Availability, Collection, ContainerCheckView, HostView};
use gpui::{
    App, Context, Div, ElementId, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Role, StatefulInteractiveElement as _, Styled as _, Window, div,
    px,
};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use super::choices::{self, LABEL_LINE_HEIGHT, LABEL_TEXT};
use crate::components::{Button, HoverHelp, Tier};
use crate::focus::Mark;
use crate::host_text::{self, HostSentence, HostText};
use crate::hosts::offers::HostKind;
use crate::prompt::{self, Answer, Inline, Question};
use crate::session::{self, ObservingCurrent};
use crate::theme::{self, metrics};

/// `.host-settings-section`'s gap between the section's parts.
const PART_GAP: f32 = 14.;
/// `.recipe-row`'s padding, top and bottom then left and right.
const ROW_PAD: (f32, f32) = (8., 10.);
/// `.recipe-row`'s gap between the host's name and its state.
const ROW_GAP: f32 = 10.;
/// The browser's default margins around the old `h4` heading and `p` status lines (1.33em and 1em at 16 pixels), which
/// set the leftovers section's spacing.
const HEADING_MARGIN: f32 = 21.;
pub(super) const LEFTOVERS_LABEL: &str = "Containers not on this helm";
const STATUS_MARGIN: f32 = 16.;

/// The section's help, the old words.
pub(super) const HELP: &str = "Containers run on your Linux hosts with rootless Podman, cgroup v2 and the systemd cgroup manager. \
                    Suspend keeps their memory in use.";
/// The leftovers' help, the old words.
pub(super) const LEFTOVERS_HELP: &str = "These containers have no completed session on this helm. Interrupted launch resources \
                              remain here for explicit cleanup. Nothing is deleted automatically.";
/// A container host's state before any check of its current address has passed or failed: the helm's own words for it
/// (`places` in `crates/farhelm-helm/src/environment/podman.rs`), which the old section showed.
const NEEDS_CHECK: &str = "this host needs a Podman check";

/// What the section shows of one list: nothing read yet, the helm's answer, or why there is none.
type Answered<T> = Option<Result<Vec<T>, HostText>>;

/// A host at an address, as a round remembers the checks it asked for: a retarget is a different address, which the
/// helm checks afresh, so it is asked about again.
type Address = (i64, Option<String>);

/// A check the helm refused, shown in place of the hosts until a newer result for its host is pushed or the host
/// leaves the pushed list ([`superseded`]).
struct Refused {
    host: i64,
    /// When the host's latest result was taken as the check was asked for (`ContainerCheckView::checked_at`); a pushed
    /// result with any other time is newer, and ends the refusal.
    checked_at: Option<u64>,
    reason: HostText,
}

/// The Containers section, a view of its own that Settings makes the first time Containers shows in an opening and
/// keeps until the dialog closes (see the module docs).
pub(super) struct Containers {
    leftovers: Answered<ContainerLeftoverView>,
    /// The first refusal among the current round's checks still not superseded by a pushed result ([`Refused`]).
    refused: Option<Refused>,
    /// The addresses this round has asked the helm to check, so following the store asks for each at most once per
    /// round: a host whose check never stores a result would otherwise be asked again on every push. `None` until the
    /// round has asked for its check of every host, which waits for the hosts list.
    asked: Option<HashSet<Address>>,
    /// The current refresh round: 1 for the reads on first showing, and one more for each Refresh. Every read and check
    /// carries the round it belongs to, and an answer from an older round is dropped, so a Refresh pressed while the
    /// previous round's checks or reads are still out cannot have its rows replaced by that round's later answers.
    round: u64,
    /// The open Delete question, drawn under the lists until it is answered.
    question: Option<Inline<ContainerLeftoverView>>,
    /// Whether a Delete is out: Refresh and every Delete are unavailable meanwhile.
    deleting: bool,
    /// Why the last Delete did not finish, in the helm's words, until the next Delete is asked.
    failure: Option<HostText>,
    /// Follows the store, to check the hosts that need it and end a superseded refusal ([`Containers::follow`]).
    _following: ObservingCurrent,
}

impl Containers {
    /// The section, reading the leftovers and checking every container host now.
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
        let mut section = Containers {
            leftovers: None,
            refused: None,
            asked: None,
            round: 0,
            question: None,
            deleting: false,
            failure: None,
            _following: session::observe_current(cx, |section: &mut Self, cx| section.follow(cx)),
        };
        section.start_round(cx);
        section
    }

    /// Refresh: read the leftovers again and check every container host again (the module docs). Does nothing while a
    /// Delete is out, when Refresh is drawn unavailable.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.deleting {
            return;
        }
        self.start_round(cx);
    }

    /// Start a new refresh round: forget the last round's refusal and checks, read the leftovers, and check every
    /// container host (once the hosts list is there).
    fn start_round(&mut self, cx: &mut Context<Self>) {
        self.round += 1;
        if self.refused.take().is_some() {
            // The refusal is drawn in place of the hosts, and nothing else on Refresh's path redraws: a keyboard press,
            // unlike a click, leaves the window as it is until the leftovers answer, up to the helm's host bound.
            cx.notify();
        }
        self.asked = None;
        Self::read_leftovers(self.round, cx);
        self.check(cx);
    }

    /// The store changed: end a refusal a newer result superseded, and check the hosts that need it (the module docs).
    fn follow(&mut self, cx: &mut Context<Self>) {
        if self
            .refused
            .as_ref()
            .is_some_and(|refused| superseded(refused, cx))
        {
            self.refused = None;
            cx.notify();
        }
        self.check(cx);
    }

    /// Ask the helm to check the container hosts this round still owes a check: every one the first time the hosts
    /// list is there, then each whose current address has no result and no check running and was not asked about yet.
    /// A refusal is kept as the first refusal of the current round still not superseded by a pushed result
    /// ([`Refused`]).
    fn check(&mut self, cx: &mut Context<Self>) {
        let Hosts::Listed(listed) = container_hosts(cx) else {
            return;
        };
        let every = self.asked.is_none();
        let asked = self.asked.get_or_insert_with(HashSet::new);
        // Remember only current addresses: the helm keeps one result per host, for its current address, so a host
        // moved away and back needs asking again, while one whose check never stores a result stays asked.
        let current: HashSet<Address> = listed
            .iter()
            .map(|(host, _)| (host.id, host.destination.clone()))
            .collect();
        asked.retain(|address| current.contains(address));
        let wanted: Vec<(Address, Option<u64>)> = listed
            .into_iter()
            .filter(|(_, check)| every || (check.checked_at.is_none() && !check.refreshing))
            .map(|(host, check)| ((host.id, host.destination.clone()), check.checked_at))
            .filter(|(address, _)| !asked.contains(address))
            .collect();
        let hosts: Vec<i64> = wanted.iter().map(|((host, _), _)| *host).collect();
        // Each host's result when its check was asked for, which tells a newer pushed result from the one it had.
        let asked_at: HashMap<i64, Option<u64>> = wanted
            .into_iter()
            .map(|(address, checked_at)| {
                let host = address.0;
                asked.insert(address);
                (host, checked_at)
            })
            .collect();
        let round = self.round;
        let section = cx.weak_entity();
        check_container_hosts(hosts, cx, move |host, reason, cx| {
            let _ = section.update(cx, |section, cx| {
                let refused = Refused {
                    host,
                    checked_at: asked_at.get(&host).copied().flatten(),
                    reason,
                };
                if section.round == round && section.refused.is_none() && !superseded(&refused, cx)
                {
                    section.refused = Some(refused);
                    cx.notify();
                }
            });
        });
    }

    /// Read the leftovers for `round`, the answer going to this section if it is still open and still on that round,
    /// under [`choices::read_route`]'s contract (once, on its own, after the calling update has returned).
    fn read_leftovers(round: u64, cx: &mut Context<Self>) {
        let section = cx.weak_entity();
        choices::read_route(
            ApiPath::api(&["container-orphans"]),
            "GET /api/container-orphans",
            cx,
            move |answer: Result<Vec<ContainerLeftoverView>, HostText>, cx| {
                let _ = section.update(cx, |section, cx| {
                    if section.round == round {
                        section.leftovers = Some(answer);
                        cx.notify();
                    }
                });
            },
        );
    }

    /// The user pressed Delete on `row` at `mark`: ask, inline, whether to delete it (the module docs).
    fn ask(
        &mut self,
        row: ContainerLeftoverView,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.deleting {
            return;
        }
        self.failure = None;
        let section = cx.weak_entity();
        let name = HostText::from_host(row.name.clone());
        let question = Question::new(
            "Delete container?",
            Answer::new(
                "Delete container",
                HoverHelp::tooltip("Delete the whole container and its files"),
                Tier::Danger,
            ),
            row,
        )
        .line(
            HostSentence::new("containers-delete-consequence")
                .text("Delete ")
                .host(&name)
                .text("? Every session, process, file and unpushed change in it will be permanently removed."),
        )
        .cancel_answer(Answer::new(
            "Cancel",
            HoverHelp::tooltip("Cancel: keep this container"),
            Tier::Neutral,
        ))
        // Deleting a container cannot be undone (SPEC.md, "Opening and closing": focus starts on Cancel then).
        .starts_on_cancel();
        self.question = Some(prompt::ask_inline(
            question,
            mark,
            move |row, _, cx| {
                let _ = section.update(cx, |section, cx| section.delete(row, cx));
            },
            window,
            cx,
        ));
        cx.notify();
    }

    /// The user confirmed deleting `row`: send its Delete (the module docs).
    fn delete(&mut self, row: ContainerLeftoverView, cx: &mut Context<Self>) {
        if self.deleting {
            return;
        }
        let request = match delete_request(&row) {
            Ok(request) => request,
            Err(error) => {
                self.failure = Some(HostText::from_host(error.to_string()));
                cx.notify();
                return;
            }
        };
        let section = cx.weak_entity();
        let sent = crate::operations::send(
            Method::Delete,
            request.path,
            request.body,
            request.shown,
            cx,
            move |answer, cx| {
                let _ = section.update(cx, |section, cx| {
                    section.deleting = false;
                    match answer {
                        Ok(_) => Self::read_leftovers(section.round, cx),
                        Err(reason) => section.failure = Some(reason),
                    }
                    cx.notify();
                });
            },
        );
        self.deleting = sent;
        cx.notify();
    }
    /// One container host's row: its name, its state as its pushed check says ([`state`]), and "· Refreshing…" while
    /// a check of it runs (`.recipe-row`).
    fn host_row(host: &HostView, check: &ContainerCheckView, index: usize, cx: &App) -> Div {
        let palette = theme::palette(cx);
        let id = format!("containers-host-{index}-state");
        let state = match state(check) {
            Some(reason) => HostSentence::new(ElementId::Name(format!("{id}-reason").into()))
                .host(&HostText::from_host(reason.to_owned())),
            None => HostSentence::new(ElementId::Name(format!("{id}-ready").into())).text("Ready"),
        };
        div()
            .flex()
            .items_center()
            .gap(px(ROW_GAP))
            .py(px(ROW_PAD.0))
            .px(px(ROW_PAD.1))
            .bg(theme::color(palette.bg_1))
            .rounded(metrics::RADIUS_SM)
            .text_size(px(LABEL_TEXT))
            .line_height(px(LABEL_LINE_HEIGHT))
            .text_color(theme::color(palette.fg_0))
            .debug_selector(move || format!("containers-host-{index}"))
            .child(host_text::element(&HostText::from_host(host.name.clone())))
            .child(
                div()
                    .id(ElementId::Name(id.into()))
                    .role(Role::Status)
                    .aria_label(state.accessible_name())
                    .min_w_0()
                    .child(state),
            )
            // No leading space: the old span's " · " lost its space at the start of its flex item, and the row's gap
            // already separates it, as it did there.
            .children(check.refreshing.then(|| div().child("· Refreshing…")))
    }

    /// One leftover's row: its name and its Delete.
    fn leftover_row(&self, row: &ContainerLeftoverView, index: usize, cx: &Context<Self>) -> Div {
        let weak = cx.weak_entity();
        let asked = row.clone();
        div()
            .flex()
            .items_center()
            .gap(px(ROW_GAP))
            .text_size(px(LABEL_TEXT))
            .line_height(px(LABEL_LINE_HEIGHT))
            .debug_selector(move || format!("containers-leftover-{index}"))
            .child(host_text::element(&HostText::from_host(row.name.clone())))
            .child(
                Button::new(
                    format!("containers-leftover-{index}-delete"),
                    "Delete",
                    HoverHelp::tooltip("Delete this container and all its processes and files"),
                    Role::Button,
                )
                .tier(Tier::Neutral)
                .disabled(self.deleting)
                .on_click(move |_, window, cx| {
                    let mark = Mark::take(cx);
                    let asked = asked.clone();
                    let _ = weak.update(cx, |section, cx| section.ask(asked, &mark, window, cx));
                }),
            )
    }

    /// A status line in the old section's paragraph look (16 pixels, the browser's default) saying `text`, under `id`
    /// and named by its words, as this area's other status lines are.
    fn status(id: &'static str, text: &'static str, cx: &App) -> gpui::Stateful<Div> {
        let palette = theme::palette(cx);
        div()
            .id(id)
            .debug_selector(move || id.to_owned())
            .role(Role::Status)
            .aria_label(text)
            .text_size(px(LABEL_TEXT))
            .line_height(px(LABEL_LINE_HEIGHT))
            .text_color(theme::color(palette.fg_0))
            .child(text)
    }

    /// A failure in the helm's words under `id`, drawn as Settings draws the helm's refusals
    /// ([`choices::host_failure_line`]): the old `.checkout-root-error` line's danger ink at the help's size.
    fn failure_line(id: &'static str, reason: &HostText, cx: &App) -> gpui::Stateful<Div> {
        choices::host_failure_line(id, HostSentence::new(id).host(reason), cx)
    }
}

impl Render for Containers {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let weak = cx.weak_entity();
        let refresh = Button::new(
            "containers-refresh",
            "Refresh",
            HoverHelp::tooltip("Refresh Podman status and containers not on this helm"),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .disabled(self.deleting)
        .on_click(move |_, _, cx| {
            let _ = weak.update(cx, |section, cx| section.refresh(cx));
        });

        // `section[aria-label="Container hosts"]`: its rows touch, as the old blocks did.
        let hosts = div()
            .id("containers-hosts")
            .role(Role::Region)
            .aria_label("Container hosts")
            .flex()
            .flex_col()
            .min_w_0();
        let hosts = match (&self.refused, container_hosts(cx)) {
            (Some(refused), _) => hosts.child(Self::failure_line(
                "containers-hosts-error",
                &refused.reason,
                cx,
            )),
            (None, Hosts::Reading) => hosts.child(Self::status(
                "containers-hosts-reading",
                "Reading container hosts…",
                cx,
            )),
            (None, Hosts::Unreadable(reason)) => {
                hosts.child(Self::failure_line("containers-hosts-error", &reason, cx))
            }
            (None, Hosts::Listed(listed)) => hosts.children(
                listed
                    .iter()
                    .enumerate()
                    .map(|(index, (host, check))| Self::host_row(host, check, index, cx)),
            ),
        };

        let heading = div()
            .id("containers-leftovers-heading")
            .role(Role::Heading)
            .aria_label(LEFTOVERS_LABEL)
            .mt(px(HEADING_MARGIN))
            .mb(px(HEADING_MARGIN))
            .text_size(px(LABEL_TEXT))
            .line_height(px(LABEL_LINE_HEIGHT))
            .font_weight(FontWeight::BOLD)
            .text_color(theme::color(palette.fg_0))
            .child(LEFTOVERS_LABEL);
        let leftovers = div()
            .id("containers-leftovers")
            .role(Role::Region)
            .aria_label("Containers not on this helm")
            .flex()
            .flex_col()
            .min_w_0()
            .child(heading)
            .child(choices::help(LEFTOVERS_HELP, cx));
        let leftovers = match &self.leftovers {
            None => leftovers.child(
                Self::status(
                    "containers-leftovers-reading",
                    "Reading container leftovers…",
                    cx,
                )
                .mt(px(STATUS_MARGIN)),
            ),
            Some(Err(reason)) => {
                leftovers.child(div().mt(px(STATUS_MARGIN)).child(Self::failure_line(
                    "containers-leftovers-error",
                    reason,
                    cx,
                )))
            }
            Some(Ok(rows)) if rows.is_empty() => leftovers.child(
                Self::status("containers-leftovers-none", "None found", cx).mt(px(STATUS_MARGIN)),
            ),
            Some(Ok(rows)) => leftovers.child(
                div()
                    .mt(px(STATUS_MARGIN))
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .children(
                        rows.iter()
                            .enumerate()
                            .map(|(index, row)| self.leftover_row(row, index, cx)),
                    ),
            ),
        };

        div()
            .id("containers")
            .role(Role::Region)
            .aria_label("Containers")
            .flex()
            .flex_col()
            .gap(px(PART_GAP))
            .w_full()
            .min_w_0()
            .child(choices::help(HELP, cx))
            .child(refresh)
            .child(hosts)
            .child(leftovers)
            .children(self.question.clone())
            .children(
                self.failure
                    .as_ref()
                    .map(|reason| Self::failure_line("containers-delete-error", reason, cx)),
            )
    }
}

/// The container hosts as the section lists them ([`container_hosts`]), borrowed from the store.
enum Hosts<'a> {
    /// The hosts collection has not arrived yet.
    Reading,
    /// The helm could not read its hosts, in its words.
    Unreadable(HostText),
    /// Each container host, in the registry's order, with its pushed check.
    Listed(Vec<(&'a HostView, &'a ContainerCheckView)>),
}

/// The pushed hosts that can hold containers ([`HostKind::holds_containers`], the rule the launcher picks container
/// hosts by too), each with the Podman check the helm pushes for exactly those hosts (the module docs), or why there
/// are none to show: the hosts collection not read yet, or read and unavailable, in which case what it lists proves
/// nothing and the helm's reason shows instead, as the old places read's failure did. A host pushed without a check,
/// which the helm never sends, is left out rather than drawn with a made-up state.
fn container_hosts(cx: &App) -> Hosts<'_> {
    let Some(store) = session::current_store(cx) else {
        return Hosts::Reading;
    };
    let collections = store.read(cx).collections();
    let Some(meta) = collections.meta(Collection::Hosts) else {
        return Hosts::Reading;
    };
    if let Availability::Unavailable { error } = &meta.availability {
        return Hosts::Unreadable(HostText::from_host(error.as_str()));
    }
    Hosts::Listed(
        collections
            .ordered::<HostView>()
            .into_iter()
            .filter(|host| HostKind::of(host).holds_containers())
            .filter_map(|host| Some((host, host.container_check.as_ref()?)))
            .collect(),
    )
}

/// The ids of the container hosts Containers lists ([`container_hosts`]), for a caller that asks for all their checks:
/// none while the hosts collection is missing, or unavailable and so possibly listing hosts that are gone.
pub(super) fn container_host_ids(cx: &App) -> Vec<i64> {
    match container_hosts(cx) {
        Hosts::Listed(listed) => listed.into_iter().map(|(host, _)| host.id).collect(),
        Hosts::Reading | Hosts::Unreadable(_) => Vec::new(),
    }
}

/// Ask the helm to check each of `hosts` again in the background (`POST /api/hosts/{id}/podman-probe`), as the old
/// Settings did when it opened and on Containers' Refresh. The one way the GUI starts a Podman check: the helm checks a
/// host on its own only when a container launch or reconnect needs it, so the GUI asks for the rest.
///
/// The contract, for every caller:
///
/// - **The caller picks the hosts**, from the pushed hosts that can hold containers: every one, or only those whose
///   current address has no result, as Containers does for a host added while it is open. Nothing is filtered here.
///   A caller must send nothing while the hosts collection is unavailable, since the entries it still lists may be
///   gone.
/// - **The result arrives through the push, not here.** The helm answers each request at once and runs the check in
///   the background; its outcome reaches the host's `container_check` in the hosts collection. So an answer only says
///   whether the check was asked for.
/// - **Each refusal goes to `on_refused`, with its host**, once per refused request, after the calling update has
///   returned; an accepted request calls nothing. What a refusal does is the caller's: Containers shows it in place of
///   its hosts until a newer result for that host is pushed, the host leaves the pushed list, or the next Refresh;
///   Recipes only logs it, since each place keeps the reason of its host's last check.
/// - **Without a UI session nothing is sent**, and nothing is called: a reset is about to close whatever asked.
pub(crate) fn check_container_hosts(
    hosts: impl IntoIterator<Item = i64>,
    cx: &mut App,
    on_refused: impl Fn(i64, HostText, &mut App) + 'static,
) {
    let on_refused = Rc::new(on_refused);
    for host in hosts {
        // A numeric segment always makes a path; the check is the type's, not a case that can happen here.
        let Ok(path) = ApiPath::api(&["hosts", &host.to_string(), "podman-probe"]) else {
            continue;
        };
        let on_refused = on_refused.clone();
        let shown = "POST /api/hosts/[host]/podman-probe";
        let sent =
            crate::operations::send(Method::Post, path, None, shown, cx, move |answer, cx| {
                if let Err(reason) = answer {
                    on_refused(host, reason, cx);
                }
            });
        if !sent {
            return;
        }
    }
}

/// Whether the push has ended `refused`: its host is no longer listed, or has a result other than the one it had when
/// the check was asked for. While the hosts list is missing or unavailable nothing is known, so the refusal stands.
fn superseded(refused: &Refused, cx: &App) -> bool {
    match container_hosts(cx) {
        Hosts::Listed(listed) => !listed
            .iter()
            .any(|(host, check)| host.id == refused.host && check.checked_at == refused.checked_at),
        Hosts::Reading | Hosts::Unreadable(_) => false,
    }
}

/// Why a host whose pushed check is `check` cannot run containers, in the helm's words, or `None` when it can (Ready):
/// the rule the helm's places read applied to the same check, its reason or, before any check of the host's current
/// address has passed or failed, [`NEEDS_CHECK`].
fn state(check: &ContainerCheckView) -> Option<&str> {
    if check.usable {
        None
    } else {
        Some(check.reason.as_deref().unwrap_or(NEEDS_CHECK))
    }
}

/// The request that deletes one leftover, as [`delete_request`] builds it.
struct DeleteRequest {
    path: ApiPath,
    body: Option<serde_json::Value>,
    /// How a refusal without words names the request.
    shown: &'static str,
}

/// The request that deletes `row`: ordinary host removal for a leftover with a host row, `DELETE
/// /api/container-orphans` with its place and canonical id for one without (the module docs).
fn delete_request(row: &ContainerLeftoverView) -> Result<DeleteRequest, PathError> {
    Ok(match row.host {
        Some(host) => DeleteRequest {
            path: ApiPath::api(&["hosts", &host.to_string()])?,
            body: None,
            shown: "DELETE /api/hosts/[host]",
        },
        None => DeleteRequest {
            path: ApiPath::api(&["container-orphans"])?,
            body: Some(serde_json::json!({ "place": row.place, "id": row.id })),
            shown: "DELETE /api/container-orphans",
        },
    })
}
