//! The launcher command field's empty list, and the shared viewport that shows typed results in its place.
//!
//! Keyboard selection belongs to the field. These rows never join the Tab order, and only a deliberate Down
//! takes Enter away from an empty field's main action. An empty-list highlight follows the setup or template
//! through store updates, so a replaced row cannot take Enter on someone else's behalf.

use farhelm_proto::RETIRED_CONTAINER_REASON;
use farhelm_proto::launcher::{LaunchTemplate, LauncherKind};
use farhelm_proto::session_requests::LaunchHistoryEntry;
use farhelm_proto::ui_channel::{HostStateView, HostView};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Context, Div, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div, px, svg,
};

use super::super::recents::{RecentRefusal, RecentSurface};
use super::{
    Drawn, Launcher, RETURN_KEY, Row, SetupSource, command_heading, command_pane_fits,
    draw_search_rows, status_line,
};
use crate::assets::{Icon, TERMINAL_FONT};
use crate::components::{HoverHelp, OptionRow, Role, Vertical, key_chip, scroll_handle};
use crate::focus::{self, Mark};
use crate::harness;
use crate::host_text::{HostSentence, HostText};
use crate::launcher::draft::harness_label;
use crate::launcher::recents::{
    ALL_DEFAULTS, destination_label, explicit_before_permissions, permission_presentation,
    selection_summary, uses_guarded_default, wrapper_summary,
};
use crate::templates::{rows, summary};
use crate::theme::{self, Palette, metrics};

/// Enough room for a setup's agent and path before they have to truncate.
pub(super) const PANE_WIDTH: f32 = 250.;

/// Keep the template's summary and shortcut together with its search-row identity.
#[allow(
    clippy::large_enum_variant,
    reason = "a few rows built per frame for the start column, never stored; boxing would only add allocations"
)]
enum StartRow {
    Recent(LaunchHistoryEntry, HostText),
    Template(rows::Row),
}

impl StartRow {
    /// Keep a host's setup tied to the offered installation even when the registry row keeps its id.
    fn key(&self, host: Option<&HostView>) -> SetupSource {
        match self {
            Self::Recent(entry, _) => SetupSource::recent(entry, host),
            Self::Template(row) => SetupSource::Template(row.name.clone()),
        }
    }

    /// The same choice that a query could offer, for the ordinary validation and application paths.
    fn result(&self) -> Row {
        match self {
            Self::Recent(entry, placement) => Row::Recent {
                entry: Box::new(entry.clone()),
                placement: placement.clone(),
            },
            Self::Template(row) => Row::Template(row.name.clone()),
        }
    }
}

impl Launcher {
    /// The empty field's choices, in display order. Keep an unavailable template catalog's words beside the rows.
    fn start_rows(&self, cx: &App) -> (Vec<StartRow>, Option<HostSentence>) {
        let mut result: Vec<_> = self
            .recent_rows(cx)
            .into_iter()
            .map(|(entry, placement)| StartRow::Recent(entry, placement))
            .collect();
        let unavailable = match rows::rows("", cx) {
            Ok(rows) => {
                result.extend(rows.into_iter().map(StartRow::Template));
                None
            }
            Err(reason) => Some(reason),
        };
        (result, unavailable)
    }

    /// From the field, only Down enters the list. Moving above its first row gives Enter back to Launch.
    pub(super) fn step_start_choice(&mut self, direction: Vertical, cx: &mut Context<Self>) {
        let (rows, _) = self.start_rows(cx);
        let host = self.chosen_host(cx);
        let from = self
            .search_box
            .start_highlight
            .as_ref()
            .and_then(|key| rows.iter().position(|row| row.key(host) == *key));
        let to = match (direction, from) {
            (Vertical::Down, None) => rows.first(),
            (Vertical::Down, Some(index)) => rows.get(index + 1).or_else(|| rows.get(index)),
            (Vertical::Up, Some(index)) => index.checked_sub(1).and_then(|i| rows.get(i)),
            (Vertical::Up, None) => None,
        };
        self.search_box.start_highlight = to.map(|row| row.key(host));
        cx.notify();
    }

    /// Resolve the presented choice again at dispatch. A withdrawn choice consumes its Enter or click and explains
    /// the refusal before the old highlight disappears.
    pub(super) fn take_start_choice(
        &mut self,
        key: &SetupSource,
        keyboard: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !command_pane_fits(window) || !self.search.text(cx).is_empty() {
            return;
        }
        let (rows, _) = self.start_rows(cx);
        if let Some(row) = rows
            .iter()
            .find(|row| row.key(self.chosen_host(cx)) == *key)
        {
            self.take_start_row(row.result(), keyboard, window, cx);
        } else {
            self.refuse_stale_choice(window, cx);
        }
    }

    /// An empty-list recent setup can submit after filling; a query result and a template only apply their choice.
    fn take_start_row(
        &mut self,
        row: Row,
        keyboard: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.row_disabled(&row, cx).is_some() {
            return;
        }
        let Row::Recent { entry, .. } = row else {
            self.take_search_row(row, window, cx);
            return;
        };
        let mark = Mark::take(cx);
        match self.take_recent(&entry, RecentSurface::CommandPane, window, cx) {
            Ok(()) => {
                self.search_box.refusal = None;
                self.close_search(window, cx);
                focus::focus_in_layer(&self.search.focus_handle(cx), &mark, window, cx);
                if keyboard {
                    self.press(window, cx);
                }
            }
            Err(RecentRefusal::Retired) => self.notice = Some(RETIRED_CONTAINER_REASON.into()),
            Err(RecentRefusal::Stale) => self.refuse_stale_choice(window, cx),
            Err(RecentRefusal::Held | RecentRefusal::KeepsEnvironment) => {}
        }
        cx.notify();
    }

    /// The left pane never has a Tab stop. Its scroll offset is independent of the form's and follows only a new
    /// keyboard highlight, so a wheel turn can reveal other choices without moving Enter's target.
    pub(in crate::launcher::view) fn command_pane(
        &mut self,
        palette: &Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let weak = cx.weak_entity();
        let empty = self.search.text(cx).is_empty();
        let results = self.search_open(window, cx);
        // Reading further down the templates must not carry that offset into a different set of matches.
        let scroll_id = if results {
            "launcher-results-scroll"
        } else {
            "launcher-start-scroll"
        };
        let scroll = scroll_handle(scroll_id.into(), window, cx);
        let list = div()
            .id(if results {
                "launcher-search-results"
            } else {
                "launcher-recents"
            })
            .debug_selector(move || {
                if results {
                    "launcher-search-results"
                } else {
                    "launcher-recents"
                }
                .into()
            })
            .flex()
            .flex_col()
            .gap(px(3.))
            .p(px(10.))
            .text_size(metrics::TEXT_MD)
            .size_full()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&scroll);
        let drawn = if results {
            draw_search_rows(self, list, &weak, window, cx)
        } else if empty {
            self.draw_start_rows(list, palette, cx)
        } else {
            Drawn {
                list,
                children: Vec::new(),
                highlighted: None,
            }
        };
        if drawn.highlighted != self.search_box.pane_revealed {
            if let Some(child) = drawn.highlighted {
                scroll.scroll_to_item(child);
            }
            self.search_box.pane_revealed = drawn.highlighted;
        }
        div()
            .id("launcher-command-pane")
            .debug_selector(|| "launcher-command-pane".into())
            .relative()
            .flex_none()
            .w(px(PANE_WIDTH))
            .min_h_0()
            .border_r_1()
            .border_color(theme::color(palette.border_dim))
            .bg(theme::color(palette.bg_1))
            .when(
                results || self.search_box.start_highlight.is_some(),
                |pane| {
                    pane.on_mouse_down_out(move |event, window, cx| {
                        let _ = weak.update(cx, |launcher, cx| {
                            if !launcher.search_box.list.contains_anchor(event.position) {
                                launcher.dismiss_command_pane(window, cx);
                            }
                        });
                    })
                },
            )
            .child(crate::layers::beside_edge(
                "launcher-command-pane-help-edge",
                window,
                cx,
            ))
            .child(drawn.list.children(drawn.children))
            .into_any_element()
    }

    /// Draw the empty list from the live catalog and history, using the same row component as typed results.
    fn draw_start_rows(&self, list: Stateful<Div>, palette: &Palette, cx: &Context<Self>) -> Drawn {
        let (rows, unavailable) = self.start_rows(cx);
        let weak = cx.weak_entity();
        let mut children = Vec::new();
        let mut highlighted = None;
        if !rows.is_empty() {
            children.push(navigation_hint(palette));
        }
        let mut previous = None;
        for (index, row) in rows.iter().enumerate() {
            let recent = matches!(row, StartRow::Recent(..));
            let taken = row.result();
            let refusal = self.row_disabled(&taken, cx);
            let disabled = refusal.is_some();
            if previous != Some(recent) {
                let (id, label) = if recent {
                    ("launcher-recent-heading", "Recent")
                } else {
                    ("launcher-template-heading", "Templates")
                };
                children.push(command_heading(id, label, palette));
                previous = Some(recent);
            }
            let key = row.key(self.chosen_host(cx));
            let active = self.search_box.start_highlight.as_ref() == Some(&key);
            let selected = active || self.search_box.source.as_ref() == Some(&key);
            if active {
                highlighted = Some(children.len());
            }
            let (id, label, help, contents) = match row {
                StartRow::Recent(entry, placement) => {
                    let id = format!("launcher-recent-{index}");
                    let whole = HostSentence::new(format!("{id}-whole"))
                        .host(&HostText::from_host(destination_label(entry)))
                        .text(" · ")
                        .host(placement)
                        .text(" · ")
                        .host(&HostText::from_host(selection_summary(&entry.selection)));
                    let label = if matches!(self.draft.mode, super::Mode::ReplaceWith { .. }) {
                        format!("Replace: {}", whole.accessible_name()).into()
                    } else {
                        whole.accessible_name()
                    };
                    let help = super::recent_hover(
                        whole,
                        entry.selection.wrapper_prefix.is_some(),
                        refusal,
                    );
                    (
                        id,
                        label,
                        help,
                        recent_choice(
                            entry,
                            placement,
                            self.chosen_host(cx),
                            "",
                            active && !disabled && entry.selection.container.is_none(),
                            selected,
                            recent_enter_cap(&self.draft.mode),
                            palette,
                        ),
                    )
                }
                StartRow::Template(template) => {
                    let mark = self.template_mark(&template.name, selected, palette, cx);
                    let (id, label, help, contents) =
                        template_choice(template, mark, active && !disabled, selected, palette);
                    (
                        id,
                        label,
                        refusal.map_or(help, HoverHelp::sentence),
                        contents,
                    )
                }
            };
            let action = weak.clone();
            children.push(
                OptionRow::new(id, label, help, Role::ListBoxOption)
                    .compact()
                    .selected(selected)
                    .disabled(disabled)
                    .child(contents)
                    .on_click(move |_, window, cx| {
                        let _ = action.update(cx, |launcher, cx| {
                            launcher.take_start_choice(&key, false, window, cx);
                        });
                    })
                    .into_any_element(),
            );
        }
        if let Some(reason) = unavailable {
            children.push(status_line(
                "launcher-template-unavailable",
                reason.accessible_name(),
                palette.warn_bright,
                reason,
            ));
        }
        Drawn {
            list: list
                .role(Role::ListBox)
                .aria_label("Recent setups and templates"),
            children,
            highlighted,
        }
    }

    /// Preserve the form's origin after manual edits. Reset clears it, and the next successful fill replaces it.
    pub(in crate::launcher::view) fn setup_source(&self, palette: &Palette) -> Option<AnyElement> {
        let label = HostSentence::new("launcher-provenance-label");
        let (label, icon) = match self.search_box.source.as_ref()? {
            SetupSource::Recent(..) => (label.text("From a recent setup"), Icon::Restart),
            SetupSource::Template(name) => (
                label
                    .text("From a template: ")
                    .host(&HostText::from_host(name.as_str())),
                Icon::Templates,
            ),
        };
        Some(
            div()
                .id("launcher-provenance")
                .debug_selector(|| "launcher-provenance".into())
                .flex()
                .flex_wrap()
                .items_center()
                .gap_x(px(8.))
                .gap_y(px(4.))
                .text_size(metrics::TEXT_SM)
                .text_color(theme::color(palette.fg_1))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(5.))
                        .min_w_0()
                        .max_w_full()
                        .px(px(6.))
                        .py(px(2.))
                        .border_1()
                        .border_color(theme::color(palette.accent_edge))
                        .rounded(metrics::RADIUS_SM)
                        .text_color(theme::color(palette.accent))
                        .child(glyph(icon, palette.accent))
                        .child(div().min_w_0().truncate().child(label)),
                )
                .child("Change anything below before launching.")
                .into_any_element(),
        )
    }
}

/// Announce the keys beside the options. The caps are examples and never consume focus.
fn navigation_hint(palette: &Palette) -> AnyElement {
    div()
        .id("launcher-command-navigation")
        .role(Role::Label)
        .aria_label("Up and Down to move, Enter to take")
        .flex()
        .items_center()
        .gap(px(4.))
        .px(px(5.))
        .text_size(metrics::TEXT_SM)
        .text_color(theme::color(palette.fg_1))
        .child(key_chip("↑", palette.fg_1, palette))
        .child(key_chip("↓", palette.fg_1, palette))
        .child("to move,")
        .child(key_chip(RETURN_KEY, palette.fg_1, palette))
        .child("to take")
        .into_any_element()
}

/// Use the session list's short destination spelling when it keeps the matching text. A query for the full
/// host name or the home prefix still needs the matching spelling. Hover help carries both full values.
fn compact_match(full: &str, compact: &str, query: &str) -> HostText {
    let query = query.to_ascii_lowercase();
    let loses_match = full.to_ascii_lowercase().contains(&query)
        && !compact.to_ascii_lowercase().contains(&query);
    HostText::from_host(if loses_match { full } else { compact })
}

/// A recent setup keeps its full destination and choices in hover help. Start a wrapper's details with its prefix,
/// since it can distinguish setups whose other visible choices are the same.
#[allow(
    clippy::too_many_arguments,
    reason = "the row's drawn facts, shared by the start column and typed results, passed through once"
)]
fn recent_choice(
    entry: &LaunchHistoryEntry,
    placement: &HostText,
    host: Option<&HostView>,
    query: &str,
    active: bool,
    selected: bool,
    enter_cap: SharedString,
    palette: &Palette,
) -> AnyElement {
    let selection = &entry.selection;
    let mark = harness::glyph(selection.harness);
    let (ink, quiet) = row_inks(selected, palette);
    let mark_ink = if selected {
        ink
    } else {
        harness::ink(mark, palette).unwrap_or(quiet)
    };
    let (permission, yolo) = permission_presentation(selection);
    let short_host = host
        .filter(|_| entry.target.is_none())
        .map_or_else(|| placement.clone(), crate::hosts::identity::short_name);
    let placement = compact_match(placement.raw(), short_host.raw(), query);
    let path = destination_label(entry);
    let folder = compact_match(&path, &crate::session_list::abbreviate_home(&path), query);
    let destination = HostSentence::new("launcher-recent-destination")
        .mono(&placement)
        .text(":")
        .mono(&folder)
        .underline_matches(query);
    let mut choices = explicit_before_permissions(selection);
    if !selection.harness.is_shell() && !yolo && !uses_guarded_default(selection) {
        choices.push(format!("Permissions: {permission}"));
    }
    if let Some(wrapper) = wrapper_summary(selection) {
        choices.insert(0, wrapper);
    }
    let words = if choices.is_empty() && !yolo {
        ALL_DEFAULTS.to_owned()
    } else {
        choices.join(" · ")
    };
    let details = HostSentence::new("launcher-recent-details")
        .host(&HostText::from_host(words))
        .underline_matches(query);
    div()
        .flex()
        .items_start()
        .gap(px(7.))
        .child(div().mt(px(2.)).child(glyph(harness::icon(mark), mark_ink)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(
                            HostSentence::new("launcher-recent-agent")
                                .text(harness_label(selection.harness))
                                .underline_matches(query),
                        )
                        .when(yolo, |line| {
                            line.child(key_chip(
                                permission,
                                if selected { ink } else { palette.danger },
                                palette,
                            ))
                        }),
                )
                .child(
                    div()
                        .truncate()
                        .text_ellipsis_middle()
                        .text_size(metrics::TEXT_SM)
                        .text_color(theme::color(quiet))
                        .font_family(TERMINAL_FONT)
                        .child(destination),
                )
                .child(
                    div()
                        .truncate()
                        .text_size(metrics::TEXT_SM)
                        .text_color(theme::color(quiet))
                        .child(details),
                ),
        )
        .when(active, |row| {
            row.child(
                key_chip(enter_cap.clone(), ink, palette)
                    .id("launcher-recent-enter")
                    .role(Role::Label)
                    .aria_label(enter_cap),
            )
        })
        .into_any_element()
}

/// The key cap a highlighted recent setup shows. In Replace with, Enter on it replaces the source session, which
/// deletes that session, so the cap names the verb, as the old recent strip's "⏎ Replace" hint did; elsewhere Enter
/// launches the setup and the bare key says enough. Only one row is highlighted at a time, so the cap's id is unique.
fn recent_enter_cap(mode: &super::Mode) -> SharedString {
    if matches!(mode, super::Mode::ReplaceWith { .. }) {
        format!("{RETURN_KEY} Replace").into()
    } else {
        RETURN_KEY.into()
    }
}

/// Explain a result's effect where the group and name alone leave it unclear. The row keeps the same full accessible
/// name as the narrow popover. Host values pass through HostSentence even when a match is underlined.
pub(super) fn typed_choice(
    launcher: &Launcher,
    row: &Row,
    words: HostSentence,
    query: &str,
    selected: bool,
    palette: &Palette,
    cx: &App,
) -> AnyElement {
    if let Row::Recent { entry, placement } = row {
        return recent_choice(
            entry,
            placement,
            launcher.chosen_host(cx),
            query,
            selected,
            selected,
            recent_enter_cap(&launcher.draft.mode),
            palette,
        );
    }
    let (ink, quiet) = row_inks(selected, palette);
    let (title, detail, mark) = match row {
        Row::Host { id, label, .. } => {
            let connected = launcher
                .store
                .read(cx)
                .collections()
                .ordered::<HostView>()
                .into_iter()
                .any(|host| {
                    host.id == *id && matches!(host.state, HostStateView::Connected { .. })
                });
            (
                HostSentence::new("launcher-result-host").host(label),
                Some(if connected {
                    "Connected · Switch host"
                } else {
                    "Switch host"
                }),
                Some((
                    if connected {
                        Icon::StatusRunning
                    } else {
                        Icon::HostCloud
                    },
                    quiet,
                )),
            )
        }
        Row::Harness(agent) => {
            let mark = harness::glyph(*agent);
            (
                HostSentence::new("launcher-result-harness").text(harness_label(*agent)),
                None,
                Some((
                    harness::icon(mark),
                    if selected {
                        ink
                    } else {
                        harness::ink(mark, palette).unwrap_or(quiet)
                    },
                )),
            )
        }
        Row::CustomAgent { .. } => {
            let mark = harness::glyph(farhelm_proto::LaunchHarness::Custom);
            (
                words,
                None,
                Some((
                    harness::icon(mark),
                    if selected {
                        ink
                    } else {
                        harness::ink(mark, palette).unwrap_or(quiet)
                    },
                )),
            )
        }
        Row::Folder(folder) => (
            HostSentence::new("launcher-result-folder").mono(&compact_match(
                folder,
                &crate::session_list::abbreviate_home(folder),
                query,
            )),
            Some("Recent folder · Start here"),
            Some((Icon::Folder, quiet)),
        ),
        Row::Template(name) => (
            HostSentence::new("launcher-result-template").host(&HostText::from_host(name.as_str())),
            Some("Apply this template"),
            Some(launcher.template_mark(name, selected, palette, cx)),
        ),
        Row::Github(_) => (
            words,
            Some("Clone into a managed checkout"),
            Some((Icon::Folder, quiet)),
        ),
        Row::UsePath(_) => (words, None, Some((Icon::Folder, quiet))),
        Row::BrowsePath(_) => (
            words,
            Some("Open the folder browser"),
            Some((Icon::Folder, quiet)),
        ),
        _ => (words, None, None),
    };
    let title = match row {
        Row::Template(_) => title.underline_template_matches(query),
        _ => title.underline_matches(query),
    };
    div()
        .flex()
        .items_start()
        .gap(px(7.))
        .when_some(mark, |row, (icon, ink)| {
            row.child(div().mt(px(2.)).child(glyph(icon, ink)))
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(div().truncate().text_ellipsis_middle().child(title))
                .when_some(detail, |body, detail| {
                    body.child(
                        div()
                            .text_size(metrics::TEXT_SM)
                            .text_color(theme::color(quiet))
                            .child(detail),
                    )
                }),
        )
        .when(selected, |row| {
            row.child(key_chip(RETURN_KEY, ink, palette))
        })
        .into_any_element()
}

impl Launcher {
    /// The harness mark says what this template sets. A command without a declared agent gets the terminal
    /// mark, and a partial template that leaves the kind open keeps the template glyph.
    fn template_mark(
        &self,
        name: &str,
        selected: bool,
        palette: &Palette,
        cx: &App,
    ) -> (Icon, theme::Token) {
        let (ink, quiet) = row_inks(selected, palette);
        let collections = self.store.read(cx).collections();
        let fields = collections
            .get::<LaunchTemplate>(name)
            .map(|template| &template.fields);
        if let Some(harness) = fields.and_then(|fields| fields.agent) {
            let mark = harness::glyph(harness);
            (
                harness::icon(mark),
                if selected {
                    ink
                } else {
                    harness::ink(mark, palette).unwrap_or(quiet)
                },
            )
        } else if fields.is_some_and(|fields| fields.kind == Some(LauncherKind::Command)) {
            (Icon::Terminal, quiet)
        } else {
            (Icon::Templates, quiet)
        }
    }
}

/// A template's summary and shortcut as in the catalog. Its quick-launch bolt sits beside the shortcut.
fn template_choice(
    template: &rows::Row,
    (icon, mark): (Icon, theme::Token),
    active: bool,
    selected: bool,
    palette: &Palette,
) -> (String, SharedString, HoverHelp, AnyElement) {
    let id = format!("launcher-template-{}", template.name);
    let name = HostText::from_host(template.name.as_str());
    let title = HostSentence::new("launcher-template-title").host(&name);
    let label = HostSentence::new("launcher-start-template-label")
        .text("Template: ")
        .host(&name);
    let (ink, quiet) = row_inks(selected, palette);
    let help = template
        .summary
        .iter()
        .fold(label.clone().text(" · "), |sentence, part| match part {
            summary::Part::Words(words) => sentence.text(words.clone()),
            summary::Part::Host(value) => sentence.host(&HostText::from_host(value.as_str())),
            summary::Part::Mono(value) => sentence.mono(&HostText::from_host(value.as_str())),
        });
    let contents = div()
        .flex()
        .items_start()
        .gap(px(7.))
        .child(div().mt(px(2.)).child(glyph(icon, mark)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(div().truncate().child(title))
                .child(
                    summary::line("launcher-template-summary", &template.summary)
                        .text_size(metrics::TEXT_SM)
                        .text_color(theme::color(quiet)),
                ),
        )
        .when(template.marked, |row| {
            row.child(glyph(
                Icon::BoltFilled,
                if selected { ink } else { palette.warn_bright },
            ))
        })
        .when_some(template.key, |row, key| {
            row.child(key_chip(key, quiet, palette))
        })
        .when(active, |row| row.child(key_chip(RETURN_KEY, ink, palette)));
    (
        id,
        label.accessible_name(),
        HoverHelp::sentence(help),
        contents.into_any_element(),
    )
}

/// Selected rows use the same inverse inks as the rest of the command pane.
fn row_inks(selected: bool, palette: &Palette) -> (theme::Token, theme::Token) {
    if selected {
        (palette.on_fill, palette.on_accent_dim)
    } else {
        (palette.fg_0, palette.fg_1)
    }
}

/// Use the same marks and asset sizes as the adjacent form and the session list.
fn glyph(icon: Icon, ink: theme::Token) -> impl IntoElement {
    svg()
        .path(icon.path())
        .size(px(13.))
        .flex_none()
        .text_color(theme::color(ink))
}
