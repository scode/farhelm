//! Settings → Containers: whether each container host can run containers, as the helm's cached Podman checks say, and
//! **Containers not on this helm**, the containers this helm owns that no completed session on it holds, each with a
//! confirmed Delete.
//!
//! SPEC.md, "Container sessions" ("Settings → Containers shows probe status per host and a refreshable **Containers not
//! on this helm** list with confirmed Delete and retryable failure"); SPEC_impl.md, "Podman places, creation and
//! images". Ported from the old app's `ContainersSettings` (`containers_settings.rs`, with the places read of
//! `environment_ui.rs`'s `use_environment_places` and the routes in `api/environments.rs`) and its look from the
//! `.host-settings-section` and `.recipe-row` rules in `app.css`, all at reference commit `24bc0580`. Here's the TLDR of
//! its rules:
//!
//! - **Read when first shown, and on Refresh.** Neither list is pushed, so the section reads both when it is first shown
//!   in an opening of the dialog (the pattern Agent trust follows), and again on Refresh, and a reply reaches only the
//!   section it was sent for. The hosts come from `GET /api/environment-places`, the helm's cached checks, which it
//!   answers without waiting on any host. Each read then asks the helm to check every container host again in the
//!   background (`POST /api/hosts/{id}/podman-probe`, which the helm answers at once) and reads the places once more,
//!   as the old section did on opening and on Refresh. A probe the helm refuses replaces the hosts with its reason, as
//!   the old read's did.
//! - **A check that is still running.** A host whose check is running reads "· Refreshing…". The helm pushes nothing
//!   when a check finishes, so after the read that follows the checks, if some host was still refreshing, the section
//!   reads the places once more [`FOLLOW_UP`] later and then stops: one bounded read, not polling. A check that takes
//!   longer than that keeps "· Refreshing…" until the next Refresh or opening. Each Refresh starts a new round, and
//!   answers and the later read of an older round are dropped (`Containers::round`). The old section polled while any
//!   host was refreshing; the migration's orchestrator chose this single read on 2026-11-15 as an interim until the
//!   helm pushes finished checks (`docs/gpui/parity/settings-and-hosts.md`, "Deviations").
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
//! NOTE: The rows show a fresh read as soon as it answers, before the checks it then asks for, where the old section
//! showed nothing new until its checks were asked for and the second read answered. Both reads answer without waiting
//! on a host, so the difference is a moment; showing the first lets the rows appear on opening without waiting behind
//! the checks.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::environment::{ContainerLeftoverView, EnvironmentPlaceView};
use gpui::{
    App, Context, Div, ElementId, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Role, StatefulInteractiveElement as _, Styled as _, Window, div,
    px,
};

use super::choices::{self, LABEL_LINE_HEIGHT, LABEL_TEXT};
use crate::components::{Button, HoverHelp, Tier};
use crate::focus::Mark;
use crate::host_text::{self, HostSentence, HostText};
use crate::prompt::{self, Answer, Inline, Question};
use crate::theme::{self, metrics};
use crate::ticker::{self, Deadline};

/// How long after the read that follows the checks the section reads the places once more, when a host was still
/// refreshing. Thirty seconds is how long a container launch waits for a host's check (`PodmanPlaces::runtime` and
/// `fresh` in `crates/farhelm-helm/src/environment/podman_probe.rs`), so a check that finishes within what the helm
/// itself treats as a reasonable wait shows by then. It is not the check's own limit: the check runs several commands
/// of up to 20 seconds each and can take longer, and then keeps "· Refreshing…" until the next Refresh or opening.
pub(super) const FOLLOW_UP: Duration = Duration::from_secs(30);

/// `.host-settings-section`'s gap between the section's parts.
const PART_GAP: f32 = 14.;
/// `.recipe-row`'s padding, top and bottom then left and right.
const ROW_PAD: (f32, f32) = (8., 10.);
/// `.recipe-row`'s gap between the host's name and its state.
const ROW_GAP: f32 = 10.;
/// The browser's default margins around the old `h4` heading and `p` status lines (1.33em and 1em at 16 pixels), which
/// set the leftovers section's spacing.
const HEADING_MARGIN: f32 = 21.;
const STATUS_MARGIN: f32 = 16.;

/// The section's help, the old words.
const HELP: &str = "Containers run on your Linux hosts with rootless Podman, cgroup v2 and the systemd cgroup manager. \
                    Suspend keeps their memory in use.";
/// The leftovers' help, the old words.
const LEFTOVERS_HELP: &str = "These containers have no completed session on this helm. Interrupted launch resources \
                              remain here for explicit cleanup. Nothing is deleted automatically.";

/// What the section shows of one list: nothing read yet, the helm's answer, or why there is none.
type Answered<T> = Option<Result<Vec<T>, HostText>>;

/// Which places read an answer belongs to, which decides what follows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlacesRead {
    /// The read on first showing or on Refresh: its answer is followed by a check of every container host.
    Fresh,
    /// The read after those checks were asked for: if a host is still refreshing, one [`PlacesRead::FollowUp`] follows
    /// [`FOLLOW_UP`] later.
    AfterChecks,
    /// That one later read, followed by nothing.
    FollowUp,
}

/// The Containers section, a view of its own that Settings makes the first time Containers shows in an opening and
/// keeps until the dialog closes (see the module docs).
pub(super) struct Containers {
    places: Answered<EnvironmentPlaceView>,
    leftovers: Answered<ContainerLeftoverView>,
    /// The current refresh round: 1 for the reads on first showing, and one more for each Refresh. Every read and check
    /// carries the round it belongs to, and an answer from an older round is dropped, so a Refresh pressed while the
    /// previous round's checks are still out can neither have its rows replaced by that round's later answers nor be
    /// followed by that round's later read.
    round: u64,
    /// The open Delete question, drawn under the lists until it is answered.
    question: Option<Inline<ContainerLeftoverView>>,
    /// Whether a Delete is out: Refresh and every Delete are unavailable meanwhile.
    deleting: bool,
    /// Why the last Delete did not finish, in the helm's words, until the next Delete is asked.
    failure: Option<HostText>,
    /// The current round's one later read of the places ([`PlacesRead::FollowUp`]). A new round drops it, and so does
    /// dropping the section with the dialog: closing Settings, or a reset, which closes it, ends the wait.
    follow_up: Option<Deadline>,
}

impl Containers {
    /// The section, reading both lists now.
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
        let mut section = Containers {
            places: None,
            leftovers: None,
            round: 0,
            question: None,
            deleting: false,
            failure: None,
            follow_up: None,
        };
        section.start_round(cx);
        section
    }

    /// Refresh: read both lists again, checking every container host again on the way (the module docs). Does nothing
    /// while a Delete is out, when Refresh is drawn unavailable.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.deleting {
            return;
        }
        self.start_round(cx);
    }

    /// Start a new refresh round: drop the previous round's later read, and read both lists.
    fn start_round(&mut self, cx: &mut Context<Self>) {
        self.round += 1;
        self.follow_up = None;
        Self::read_places(self.round, PlacesRead::Fresh, cx);
        Self::read_leftovers(self.round, cx);
    }

    /// Read the places for `round`, the answer going to this section if it is still open and still on that round, under
    /// [`choices::read_route`]'s contract (once, on its own, after the calling update has returned).
    fn read_places(round: u64, read: PlacesRead, cx: &mut Context<Self>) {
        let section = cx.weak_entity();
        choices::read_route(
            ApiPath::api(&["environment-places"]),
            "GET /api/environment-places",
            cx,
            move |answer: Result<Vec<EnvironmentPlaceView>, HostText>, cx| {
                let _ = section.update(cx, |section, cx| {
                    if section.round == round {
                        section.places_answered(read, answer, cx);
                    }
                });
            },
        );
    }

    /// Read the leftovers for `round`, as [`Containers::read_places`] reads the places.
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

    /// A places read of the current round answered: show it, and go on as `read` says (see [`PlacesRead`]).
    fn places_answered(
        &mut self,
        read: PlacesRead,
        answer: Result<Vec<EnvironmentPlaceView>, HostText>,
        cx: &mut Context<Self>,
    ) {
        let hosts: Vec<i64> = answer
            .as_ref()
            .map(|places| {
                places
                    .iter()
                    .filter_map(|place| place.place.container_host())
                    .collect()
            })
            .unwrap_or_default();
        let refreshing = answer
            .as_ref()
            .is_ok_and(|places| container_hosts(places).any(|place| place.refreshing));
        self.places = Some(answer);
        match read {
            PlacesRead::Fresh => self.check(hosts, cx),
            PlacesRead::AfterChecks if refreshing => {
                let (section, round) = (cx.weak_entity(), self.round);
                self.follow_up = Some(ticker::after(FOLLOW_UP, cx, move |cx| {
                    let _ = section.update(cx, |_, cx| {
                        Self::read_places(round, PlacesRead::FollowUp, cx);
                    });
                }));
            }
            PlacesRead::AfterChecks | PlacesRead::FollowUp => {}
        }
        cx.notify();
    }

    /// Ask the helm to check each of `hosts` again in the background, then read the places once more; a check the helm
    /// refuses replaces the hosts with its reason instead, as the old read's did. Nothing is asked when no host can
    /// run containers. Answers that arrive after a newer round started change nothing.
    fn check(&mut self, hosts: Vec<i64>, cx: &mut Context<Self>) {
        let paths: Result<Vec<ApiPath>, PathError> = hosts.into_iter().map(probe_path).collect();
        let paths = match paths {
            Ok(paths) if paths.is_empty() => return,
            Ok(paths) => paths,
            Err(error) => {
                self.places = Some(Err(HostText::from_host(error.to_string())));
                return;
            }
        };
        let round = self.round;
        // Shared by every check's answer: how many are still out, and the first refusal.
        let outstanding = Rc::new(Cell::new(paths.len()));
        let refused = Rc::new(RefCell::new(None::<HostText>));
        for path in paths {
            let section = cx.weak_entity();
            let (outstanding, refused) = (outstanding.clone(), refused.clone());
            let sent = crate::operations::send(
                Method::Post,
                path,
                None,
                "POST /api/hosts/[host]/podman-probe",
                cx,
                move |answer, cx| {
                    if let Err(reason) = answer {
                        refused.borrow_mut().get_or_insert(reason);
                    }
                    outstanding.set(outstanding.get().saturating_sub(1));
                    if outstanding.get() > 0 {
                        return;
                    }
                    let _ = section.update(cx, |section, cx| {
                        if section.round != round {
                            return;
                        }
                        match refused.borrow_mut().take() {
                            Some(reason) => {
                                section.places = Some(Err(reason));
                                cx.notify();
                            }
                            None => Self::read_places(round, PlacesRead::AfterChecks, cx),
                        }
                    });
                },
            );
            if !sent {
                // No UI session: nothing will answer, and a reset is about to close the dialog anyway.
                return;
            }
        }
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
            Answer {
                label: "Delete container".into(),
                help: HoverHelp::tooltip("Delete the whole container and its files"),
                tier: Tier::Danger,
            },
            row,
        )
        .line(
            HostSentence::new("containers-delete-consequence")
                .text("Delete ")
                .host(&name)
                .text("? Every session, process, file and unpushed change in it will be permanently removed."),
        )
        .cancel_answer(Answer {
            label: "Cancel".into(),
            help: HoverHelp::tooltip("Cancel: keep this container"),
            tier: Tier::Neutral,
        })
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

    /// One container host's row: its name, its state, and "· Refreshing…" while its check runs (`.recipe-row`).
    fn host_row(place: &EnvironmentPlaceView, index: usize, cx: &App) -> Div {
        let palette = theme::palette(cx);
        let id = format!("containers-host-{index}-state");
        let state = match &place.reason {
            Some(reason) => HostSentence::new(ElementId::Name(format!("{id}-reason").into()))
                .host(&HostText::from_host(reason.clone())),
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
            .child(host_text::element(&HostText::from_host(place.name.clone())))
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
            .children(place.refreshing.then(|| div().child("· Refreshing…")))
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
        let hosts = match &self.places {
            None => hosts.child(Self::status(
                "containers-hosts-reading",
                "Reading container hosts…",
                cx,
            )),
            Some(Err(reason)) => {
                hosts.child(Self::failure_line("containers-hosts-error", reason, cx))
            }
            Some(Ok(places)) => hosts.children(
                container_hosts(places)
                    .enumerate()
                    .map(|(index, place)| Self::host_row(place, index, cx)),
            ),
        };

        let heading = div()
            .id("containers-leftovers-heading")
            .role(Role::Heading)
            .aria_label("Containers not on this helm")
            .mt(px(HEADING_MARGIN))
            .mb(px(HEADING_MARGIN))
            .text_size(px(LABEL_TEXT))
            .line_height(px(LABEL_LINE_HEIGHT))
            .font_weight(FontWeight::BOLD)
            .text_color(theme::color(palette.fg_0))
            .child("Containers not on this helm");
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

/// The places that are container hosts, in the helm's order: the Tensorlake account is a place too, and has its own
/// section.
fn container_hosts(places: &[EnvironmentPlaceView]) -> impl Iterator<Item = &EnvironmentPlaceView> {
    places
        .iter()
        .filter(|place| place.place.container_host().is_some())
}

/// `POST /api/hosts/{host}/podman-probe`: check `host` again in the background.
fn probe_path(host: i64) -> Result<ApiPath, PathError> {
    ApiPath::api(&["hosts", &host.to_string(), "podman-probe"])
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
