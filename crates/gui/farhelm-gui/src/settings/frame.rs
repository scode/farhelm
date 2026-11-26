//! Settings keeps navigation and the current page in separate scroll regions.
//! The frame pins the controls common to every page while Tab still reveals controls inside either pane.

use farhelm_proto::ui_channel::Preferences;
use gpui::{
    AnyElement, Context, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    Role, ScrollHandle, StatefulInteractiveElement as _, Styled as _, Window, canvas, div,
    prelude::FluentBuilder as _, px, svg,
};

use super::{SECTION_GAP, Section, SettingsDialog, TrustAgent};
use crate::assets::Icon;
use crate::components::{self, Button, HoverHelp, Tier, scroll_from_controls, scroll_handle};
use crate::{focus, layers, theme};

/// Settings' visual grouping determines the tile hue, independent of the page's storage scope.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Group {
    General,
    Placement,
    Agents,
    CustomAgents,
}

impl Group {
    const ALL: [Self; 4] = [
        Self::General,
        Self::Placement,
        Self::Agents,
        Self::CustomAgents,
    ];

    /// Group and section order agree with the order used by the search catalog.
    fn of(section: &Section) -> Self {
        match section {
            Section::Sessions
            | Section::Appearance
            | Section::Terminal
            | Section::Notifications
            | Section::Credentials
            | Section::App => Self::General,
            Section::Hosts
            | Section::Checkouts
            | Section::Containers
            | Section::Recipes
            | Section::Tensorlake => Self::Placement,
            Section::Agent(_) => Self::Agents,
            Section::CustomAgent(_) | Section::AddCustomAgent => Self::CustomAgents,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Placement => "Where sessions run",
            Self::Agents => "Agents",
            Self::CustomAgents => "Custom agents",
        }
    }

    fn hue(self, palette: &theme::Palette) -> theme::Token {
        match self {
            Self::General => palette.settings_general(),
            Self::Placement => palette.settings_placement(),
            Self::Agents => palette.settings_agents(),
            Self::CustomAgents => palette.harness[8],
        }
    }
}

/// The heading names the installations the page configures. App includes the helm when release highlights are shown.
enum Scope {
    Hosts,
    Helm,
    Device,
    DeviceAndHelm,
}

impl Scope {
    fn of(section: &Section, cx: &gpui::App) -> Self {
        match section {
            Section::Recipes | Section::Tensorlake | Section::Credentials => Self::Helm,
            Section::App if section.has_live_choices(cx) => Self::DeviceAndHelm,
            Section::App => Self::Device,
            Section::Sessions
            | Section::Appearance
            | Section::Terminal
            | Section::Notifications
            | Section::Hosts
            | Section::Checkouts
            | Section::Containers
            | Section::Agent(_)
            | Section::CustomAgent(_)
            | Section::AddCustomAgent => Self::Hosts,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Hosts => "Every host on this helm",
            Self::Helm => "This helm",
            Self::Device => "This Mac",
            Self::DeviceAndHelm => "This Mac and this helm",
        }
    }

    fn icon(&self) -> Icon {
        match self {
            Self::Hosts => Icon::Hosts,
            Self::Helm => Icon::HostHouse,
            Self::Device | Self::DeviceAndHelm => Icon::HostLaptop,
        }
    }
}

impl Section {
    /// The page description belongs in the pinned heading so it remains available on long or scrolled forms.
    fn description(&self) -> gpui::SharedString {
        let description = match self {
            Self::Sessions => "How session commands and quick launches behave",
            Self::Appearance => "The interface colors",
            Self::Terminal => "Links and files clicked in a terminal",
            Self::Notifications => "Sounds, push alerts and Farhelm announcements",
            Self::App => "This Farhelm app's version and updates",
            Self::Hosts => "Updating, adding and removing hosts",
            Self::Checkouts => "Where managed checkouts go and how they start",
            Self::Containers => "Podman hosts and leftover containers",
            Self::Recipes => "Software and credentials for new containers and sandboxes",
            Self::Tensorlake => "Cloud sandboxes in your Tensorlake project",
            Self::Agent(harness) => match (
                TrustAgent::for_harness(*harness),
                harness.agent_kind().offers_persistent_hooks(),
            ) {
                (Some(_), true) => "Workspace trust and conversation hooks",
                (Some(agent), false) => agent.page_description(),
                (None, true) => "Conversation hooks for Restart",
                (None, false) => "",
            },
            Self::Credentials => "Secrets Farhelm hands to hosts for you",
            Self::CustomAgent(id) => return format!("Custom agent · ID {id}").into(),
            Self::AddCustomAgent => "Register an agent command for this helm",
        };
        description.into()
    }

    /// Pages with shared choices explain live updates once in the scope label's hover help.
    fn has_live_choices(&self, cx: &gpui::App) -> bool {
        match self {
            Self::App => crate::desktop::updates::upgrade_notes(cx).is_some(),
            Self::Agent(harness) => {
                harness.agent_kind().offers_persistent_hooks()
                    || TrustAgent::for_harness(*harness)
                        .and_then(TrustAgent::hint)
                        .is_some()
            }
            Self::Sessions
            | Self::Appearance
            | Self::Terminal
            | Self::Notifications
            | Self::Hosts
            | Self::Checkouts
            | Self::Tensorlake => true,
            Self::Containers
            | Self::Recipes
            | Self::Credentials
            | Self::CustomAgent(_)
            | Self::AddCustomAgent => false,
        }
    }

    /// Use the same glyph and role in the 20px section tile and the 32px page tile.
    /// The selected tile gets inverse ink so it stays legible on the accent fill.
    fn tile(&self, large: bool, selected: bool, palette: &theme::Palette) -> Div {
        let (side, corner, glyph) = if large {
            (32., 6., 17.)
        } else {
            (20., 4., 13.)
        };
        let hue = Group::of(self).hue(palette);
        let tile_ink = theme::color(if selected { palette.on_fill } else { hue });
        // Group color belongs to the tile. Agent glyphs keep the same ink as their marks in the session list.
        let glyph_ink = theme::color(match self {
            _ if selected => palette.on_fill,
            Self::Agent(harness) => crate::harness::ink(crate::harness::glyph(*harness), palette)
                .unwrap_or(palette.fg_1),
            _ => hue,
        });
        div()
            .flex_none()
            .size(px(side))
            .rounded(px(corner))
            .bg(tile_ink.opacity(if selected { 0.20 } else { 0.24 }))
            .flex()
            .items_center()
            .justify_center()
            .child(
                svg()
                    .path(self.icon().path())
                    .size(px(glyph))
                    .text_color(glyph_ink),
            )
    }
}

impl SettingsDialog {
    /// The search field stays reachable even when the section list itself has to scroll.
    pub(super) fn navigation(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = theme::palette(cx);
        let help_edge =
            layers::beside_edge("settings-navigation-help-edge", window, cx).into_any_element();
        let scroll = scroll_handle("settings-navigation-scroll".into(), window, cx);
        let searching = self.search.active();
        let registered = self.custom_sections(cx);
        let sections = Group::ALL.into_iter().filter_map(|group| {
            let rows: Vec<_> = registered
                .iter()
                .cloned()
                .chain(Section::all())
                .filter(|section| Group::of(section) == group && section.listed(cx))
                .filter_map(|section| {
                    let count = searching.then(|| self.search.section_count(&section));
                    (count != Some(0)).then(|| self.section_button(section, count, cx))
                })
                .collect();
            (!rows.is_empty()).then(|| {
                div()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .id(("settings-group", group as usize))
                            .role(Role::Label)
                            .aria_label(group.label())
                            .px(px(8.))
                            .pt(px(8.))
                            .pb(px(3.))
                            .text_size(px(10.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::color(palette.fg_1))
                            .child(group.label().to_uppercase()),
                    )
                    .children(rows)
            })
        });
        let list = div()
            .id("settings-navigation-region")
            .debug_selector(|| "settings-navigation-region".into())
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(px(2.))
            .px(px(3.))
            .pb(px(4.))
            .overflow_y_scroll()
            .children(sections);
        div()
            .id("settings-sections")
            .relative()
            .role(Role::Navigation)
            .aria_label("Settings sections")
            .flex_none()
            .w(px(178.))
            .min_h_0()
            .flex()
            .flex_col()
            .p(px(8.))
            .bg(theme::color(palette.bg_1))
            .border_r_1()
            .border_color(theme::color(palette.border_dim))
            .child(help_edge)
            .child(
                div()
                    .flex_none()
                    .px(px(3.))
                    .child(self.search.field(self.platform, cx)),
            )
            .child(focus::scroll_region(
                scroll_from_controls(list, &scroll),
                &scroll,
            ))
            .into_any_element()
    }

    /// The shared button keeps the platform's focus, tooltip and selection behavior for both ordinary and search rows.
    fn section_button(&self, section: Section, count: Option<usize>, cx: &Context<Self>) -> Button {
        let weak = cx.weak_entity();
        let palette = theme::palette(cx);
        let selected = count.is_none() && section == self.shown;
        let edited = self.section_edited(&section, cx);
        let display = section.display_label(cx);
        let label = display.accessible_text();
        let content = div()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .gap(px(7.))
            .child(section.tile(false, selected, palette))
            .child(div().flex_1().min_w_0().truncate().child(display))
            .when_some(count, |row, count| {
                row.child(
                    div()
                        .flex_none()
                        .text_size(theme::metrics::TEXT_SM)
                        .child(count.to_string()),
                )
            })
            .children(edited.then(|| super::editing::dot(selected, palette)));
        Button::new(
            section.nav_button_id(),
            label.clone(),
            section.nav_help(&label, cx).beside_list(false),
            Role::Button,
        )
        .tier(Tier::Nav)
        .pressed(selected)
        .content(content)
        .when_some(count, |button, count| {
            let matches = if count == 1 { "match" } else { "matches" };
            button.accessible_name(format!("{label} ({count} {matches})"))
        })
        .when(edited, |button| {
            let name = count.map_or_else(
                || format!("{label} (unsaved changes)"),
                |count| {
                    let matches = if count == 1 { "match" } else { "matches" };
                    format!("{label} ({count} {matches}, unsaved changes)")
                },
            );
            button.accessible_name(name)
        })
        .on_click(move |_, window, cx| {
            let _ = weak.update(cx, |dialog, cx| {
                dialog.search.clear(window, cx);
                dialog.show(section.clone(), window, cx);
            });
        })
    }

    /// The pinned heading and the page's scroll viewport are siblings, so focus reveal cannot scroll under the heading.
    pub(super) fn page(
        &self,
        preferences: Option<&Preferences>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = theme::palette(cx);
        let scroll = div()
            .id("settings-body")
            .debug_selector(|| "settings-body".into())
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .px(px(20.))
                    .pt(px(10.))
                    .pb(px(16.))
                    .gap(px(SECTION_GAP))
                    .children(self.body(self.shown.clone(), preferences, window, cx)),
            );
        div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(theme::color(palette.settings_page))
            .child(self.page_heading(window, cx))
            .child(focus::scroll_region(
                scroll_from_controls(scroll, &self.page_scroll),
                &self.page_scroll,
            ))
            .into_any_element()
    }

    /// The scope tells readers where the controls apply without repeating that information under each switch.
    fn page_heading(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let section = &self.shown;
        let scope = Scope::of(section, cx);
        let display = if *section == Section::AddCustomAgent {
            "Add custom agent".into()
        } else {
            section.display_label(cx)
        };
        let description = section.description();
        let palette = theme::palette(cx);
        let scope_id = gpui::SharedString::from("settings-page-scope");
        let scope_line = div()
            .id(gpui::ElementId::Name(scope_id.clone()))
            .role(Role::Label)
            .aria_label(scope.label())
            .flex_none()
            .flex()
            .items_center()
            .gap(px(5.))
            .text_size(px(11.))
            .text_color(theme::color(palette.fg_1))
            .child(
                svg()
                    .path(scope.icon().path())
                    .size(px(13.))
                    .text_color(theme::color(palette.fg_1)),
            )
            .child(scope.label());
        let scope_line = if section.has_live_choices(cx) {
            let help = HoverHelp::tooltip(
                "Changes from the command line or an agent show here right away.",
            );
            components::with_hover_help(scope_line, &scope_id, &help, true, None, window, cx)
        } else {
            scope_line
        };
        heading_band(&self.page_scroll, palette)
            .items_center()
            .gap(px(12.))
            .child(section.tile(true, false, palette))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    // Give the description the whole line, even where the window caps the dialog's width.
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(8.))
                            .child(
                                div()
                                    .id("settings-heading")
                                    .debug_selector(|| "settings-heading".into())
                                    .role(Role::Heading)
                                    .aria_label(display.accessible_text())
                                    .aria_description(description.clone())
                                    // Keep the scope in view even when a registration name takes the rest of the row.
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(16.))
                                    .line_height(px(20.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(display),
                            )
                            .child(scope_line),
                    )
                    .when(!description.is_empty(), |heading| {
                        heading.child(
                            div()
                                .text_size(px(12.))
                                .line_height(px(16.))
                                .text_color(theme::color(palette.fg_1))
                                .truncate()
                                .child(description),
                        )
                    }),
            )
            .into_any_element()
    }
}

/// Search and ordinary pages keep the same pinned edge above their own scroll viewports.
pub(super) fn heading_band(scroll: &ScrollHandle, palette: &theme::Palette) -> gpui::Div {
    div()
        .relative()
        .flex_none()
        .flex()
        .px(px(22.))
        .pt(px(14.))
        .pb(px(12.))
        .child(page_rule(scroll, palette))
}

/// Paint the heading's edge after layout clamps the page's scroll offset. A wheel event on a short page or a resize
/// that fits all the content can leave an out-of-range offset until the viewport is laid out.
fn page_rule(scroll: &ScrollHandle, palette: &theme::Palette) -> impl IntoElement {
    let scroll = scroll.clone();
    let border = theme::color(palette.border_dim);
    let shade = theme::color(palette.shadow_menu);
    let shadow = gpui::linear_gradient(
        180.,
        gpui::linear_color_stop(shade.opacity(0.5), 0.),
        gpui::linear_color_stop(shade.opacity(0.), 1.),
    );
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            if scroll.offset().y < px(0.) {
                let mut below = bounds;
                below.origin.y += bounds.size.height;
                below.size.height = px(8.);
                window.paint_quad(gpui::fill(below, shadow));
                window.paint_quad(gpui::fill(bounds, border));
            }
        },
    )
    .absolute()
    .bottom_0()
    .left_0()
    .w_full()
    .h(px(1.))
}
