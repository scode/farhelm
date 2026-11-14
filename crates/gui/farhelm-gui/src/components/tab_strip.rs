//! The tab strip: a few tabs that switch which panel of a dialog shows, as the session launcher's Agent and Command
//! tabs did.
//!
//! The old launcher drew them as a `role="tablist"` of `role="tab"` buttons (`.launch-kind-tabs` in `app.css`, and
//! `create_form.rs` around line 8767, at reference commit `24bc0580`). Here's the TLDR of what it keeps:
//!
//! - **One tab stop, the chosen tab.** Tab enters the strip at the chosen tab and the next Tab leaves it (the old roving
//!   `tabindex`).
//! - **The arrows switch tabs, and focus follows the choice.** Left and Right move to the previous and next tab,
//!   wrapping at the ends (the old strip had two tabs and swapped between them); Home and End go to the first and last.
//!   Each also chooses the tab it lands on, as the old strip did.
//! - **Enter chooses, then runs the owner's main action**, through [`TabStrip::on_enter`]: the old launcher's
//!   `enter_choice`, where Enter on a launch-kind tab applied it and launched (SPEC.md, "Creation": "Enter on a choice
//!   button (launch-kind tab, ...) applies that choice and then launches"). A held Enter does nothing, so holding it
//!   never launches twice. Without `on_enter`, Enter chooses. Space and a click choose.
//! - **The look**: a row of tabs over a one-pixel rule, the chosen tab filled with the accent and its ink inverted, the
//!   others quiet text, 14-pixel semibold words (`.launch-kind-tab`).
//!
//! It holds no state: its owner says which tab is chosen and hears each choice. Focus follows the platform's control
//! rule ([`crate::focus::OnPress::Control`]), as the other choice controls do.
//!
//! Built for one consumer, the launcher, and documented for the next: a second tab strip (the session view's terminal
//! tabs, say) extends this one rather than adding another.

use std::rc::Rc;

use gpui::{
    App, ElementId, FocusHandle, FontWeight, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};

use super::HoverHelp;
use crate::focus;
use crate::theme::{self, metrics};

/// `.launch-kind-tab`: `padding: 6px 14px`.
const TAB_PAD: (f32, f32) = (6., 14.);
/// `.launch-kind-tabs`: `gap: 4px`.
const TAB_GAP: f32 = 4.;
/// `.launch-kind-tab`: 14-pixel words.
const TAB_TEXT: f32 = 14.;

/// What a choice does with the chosen tab's value.
type OnChoose<V> = Rc<dyn Fn(&V, &mut Window, &mut App)>;

/// The owner's main action, which Enter on a tab runs after choosing it.
type OnEnter = Rc<dyn Fn(&mut Window, &mut App)>;

/// One tab: its words (drawn, and its accessible name), its hover help and the value its owner hears.
pub struct TabOption<V> {
    label: SharedString,
    help: HoverHelp,
    value: V,
}

impl<V> TabOption<V> {
    pub fn new(label: impl Into<SharedString>, help: HoverHelp, value: V) -> Self {
        TabOption {
            label: label.into(),
            help,
            value,
        }
    }
}

/// A strip of tabs. See the module docs.
#[derive(IntoElement)]
pub struct TabStrip<V: Clone + PartialEq + 'static> {
    id: SharedString,
    label: SharedString,
    help: HoverHelp,
    tabs: Vec<TabOption<V>>,
    chosen: Option<V>,
    on_choose: Option<OnChoose<V>>,
    on_enter: Option<OnEnter>,
}

impl<V: Clone + PartialEq + 'static> TabStrip<V> {
    /// A strip labelled `label` (the tab list's accessible name, "Launch kind") with hover help `help`. The role is
    /// always the tab list's; its tabs report the tab role and whether they are selected.
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        help: HoverHelp,
    ) -> Self {
        TabStrip {
            id: id.into(),
            label: label.into(),
            help,
            tabs: Vec::new(),
            chosen: None,
            on_choose: None,
            on_enter: None,
        }
    }

    /// Add a tab, after the ones already added.
    pub fn tab(mut self, tab: TabOption<V>) -> Self {
        self.tabs.push(tab);
        self
    }

    /// Which tab is chosen.
    pub fn chosen(mut self, chosen: V) -> Self {
        self.chosen = Some(chosen);
        self
    }

    /// What choosing a tab does, with the tab's value.
    pub fn on_choose(mut self, handler: impl Fn(&V, &mut Window, &mut App) + 'static) -> Self {
        self.on_choose = Some(Rc::new(handler));
        self
    }

    /// The owner's main action, which Enter on a tab runs after choosing it (see the module docs).
    pub fn on_enter(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_enter = Some(Rc::new(handler));
        self
    }
}

/// What a tab's key and click handlers share.
struct Strip<V> {
    values: Vec<V>,
    handles: Vec<FocusHandle>,
    on_choose: Option<OnChoose<V>>,
    on_enter: Option<OnEnter>,
}

impl<V> Strip<V> {
    fn choose(&self, index: usize, window: &mut Window, cx: &mut App) {
        if let Some(handler) = &self.on_choose {
            handler(&self.values[index], window, cx);
        }
    }

    /// Move focus from tab `from` to tab `to` and choose it.
    fn go(&self, from: usize, to: usize, window: &mut Window, cx: &mut App) {
        focus::move_within_control(&self.handles[from], &self.handles[to], window, cx);
        self.choose(to, window, cx);
    }

    /// A key on the focused tab `index`. A modified key is not an activation (the controls' shared key rule) and goes
    /// on to the owner.
    fn key(&self, index: usize, event: &KeyDownEvent, window: &mut Window, cx: &mut App) {
        if event.keystroke.modifiers.modified() {
            return;
        }
        let last = self.values.len() - 1;
        match event.keystroke.key.as_str() {
            "enter" => {
                // Taken even when held, so a held Enter neither launches again nor reaches what holds the strip.
                cx.stop_propagation();
                if !event.is_held {
                    self.choose(index, window, cx);
                    if let Some(on_enter) = &self.on_enter {
                        on_enter(window, cx);
                    }
                }
            }
            "space" if !event.is_held => {
                cx.stop_propagation();
                self.choose(index, window, cx);
            }
            "left" => {
                cx.stop_propagation();
                self.go(index, if index == 0 { last } else { index - 1 }, window, cx);
            }
            "right" => {
                cx.stop_propagation();
                self.go(index, if index == last { 0 } else { index + 1 }, window, cx);
            }
            "home" => {
                cx.stop_propagation();
                self.go(index, 0, window, cx);
            }
            "end" => {
                cx.stop_propagation();
                self.go(index, last, window, cx);
            }
            _ => {}
        }
    }
}

impl<V: Clone + PartialEq + 'static> RenderOnce for TabStrip<V> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = theme::palette(cx);
        let ids: Vec<SharedString> = self
            .tabs
            .iter()
            .map(|tab| format!("{}-{}", self.id, tab.label).into())
            .collect();
        let chosen = self
            .tabs
            .iter()
            .position(|tab| self.chosen.as_ref() == Some(&tab.value))
            .unwrap_or(0);
        let handles: Vec<FocusHandle> = ids
            .iter()
            .map(|id| {
                focus::control_handle(ElementId::Name(format!("{id}-focus").into()), window, cx)
            })
            .collect();
        // One tab stop: the focused tab while focus is in the strip, so the next Tab leaves it, else the chosen one.
        let stop = handles
            .iter()
            .position(|handle| handle.is_focused(window))
            .unwrap_or(chosen);
        let handles: Vec<FocusHandle> = handles
            .into_iter()
            .enumerate()
            .map(|(index, handle)| handle.tab_stop(index == stop))
            .collect();
        let strip = Rc::new(Strip {
            values: self.tabs.iter().map(|tab| tab.value.clone()).collect(),
            handles: handles.clone(),
            on_choose: self.on_choose.clone(),
            on_enter: self.on_enter.clone(),
        });
        let mut tabs = Vec::with_capacity(self.tabs.len());
        for (index, tab) in self.tabs.iter().enumerate() {
            let selected = index == chosen;
            let ring = focus::shows_ring(&handles[index], window);
            let element = div()
                .id(ElementId::Name(ids[index].clone()))
                // Tests find a control by its id; this records nothing outside test builds.
                .debug_selector(|| ids[index].to_string())
                .role(Role::Tab)
                .aria_label(tab.label.clone())
                .aria_selected(selected)
                .py(px(TAB_PAD.0))
                .px(px(TAB_PAD.1))
                .rounded_tl(metrics::RADIUS_SM)
                .rounded_tr(metrics::RADIUS_SM)
                .text_size(px(TAB_TEXT))
                .font_weight(FontWeight::SEMIBOLD)
                .cursor_pointer()
                .when(selected, |tab| {
                    tab.bg(theme::color(palette.accent_fill))
                        .text_color(theme::color(palette.on_fill))
                })
                .when(!selected, |tab| tab.text_color(theme::color(palette.fg_1)))
                .relative()
                .child(tab.label.clone())
                // The keyboard ring, drawn over the tab since GPUI has no outline, so it moves nothing: the inverse ink
                // on the chosen tab's fill, the accent elsewhere, as the other controls' rings.
                .when(ring, |tab| {
                    tab.child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .right_0()
                            .bottom_0()
                            .border_2()
                            .rounded_tl(metrics::RADIUS_SM)
                            .rounded_tr(metrics::RADIUS_SM)
                            .border_color(theme::color(if selected {
                                palette.on_fill
                            } else {
                                palette.accent
                            })),
                    )
                });
            let element = super::with_hover_help(element, &ids[index], &tab.help, true, window, cx);
            let (on_key, on_click) = (strip.clone(), strip.clone());
            let element = focus::track(element, &handles[index], focus::OnPress::Control)
                .on_key_down(move |event, window, cx| on_key.key(index, event, window, cx));
            let on_click: super::Handler =
                Rc::new(move |_, window, cx| on_click.choose(index, window, cx));
            tabs.push(super::activate_on_click(element, Some(on_click)));
        }
        // The strip's help is its accessible description only. A hover listener on the strip would be heard after the
        // tabs' and replace their tooltips (GPUI tells hover listeners from the last drawn to the first; the radio group
        // works around the same trap with a surface laid over its options), and each tab already has its own.
        div()
            .id(ElementId::Name(self.id.clone()))
            .debug_selector(|| self.id.to_string())
            .role(Role::TabList)
            .aria_label(self.label.clone())
            .aria_description(self.help.description())
            .flex()
            .gap(px(TAB_GAP))
            .border_b_1()
            .border_color(theme::color(palette.border))
            .children(tabs)
    }
}
