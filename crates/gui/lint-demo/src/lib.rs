//! One deliberate violation per ban in `crates/gui/clippy.toml`, plus the unwrap/expect denies.
//!
//! Clippy matches banned paths by name, so a GPUI or GPUI Kit upgrade that moves or renames an item makes its ban
//! stop matching without any error. `scripts/check-gui-lints.py` lints this crate with the `violations` feature and
//! fails unless every `// expect:` line below gets its diagnostic inside the function that carries it, and unless every
//! ban in the clippy.toml has such a line. Nothing here is ever called; it only has to type-check.
//!
//! Each demo function holds exactly one violation and starts with `// expect: <lint> <path>`, which the script reads.
#![cfg(feature = "violations")]
#![allow(
    dead_code,
    unused_imports,
    unused_must_use,
    clippy::let_underscore_future
)]

use std::time::Duration;

use gpui::{
    App, AsyncWindowContext, BackgroundExecutor, Context, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Styled, Task, TaskExt, Window, div,
};
use gpui_component::button::{Button, Toggle};
use gpui_component::menu::{ContextMenuExt, DropdownMenu};
use gpui_component::{Placement, WindowExt};

// Focus moves.

fn window_focus(window: &mut Window, handle: &FocusHandle, cx: &mut App) {
    // expect: disallowed_methods gpui::Window::focus
    window.focus(handle, cx);
}

fn window_blur(window: &mut Window, cx: &mut App) {
    // expect: disallowed_methods gpui::Window::blur
    window.blur(cx);
}

fn window_focus_next(window: &mut Window, cx: &mut App) {
    // expect: disallowed_methods gpui::Window::focus_next
    window.focus_next(cx);
}

fn window_focus_prev(window: &mut Window, cx: &mut App) {
    // expect: disallowed_methods gpui::Window::focus_prev
    window.focus_prev(cx);
}

fn window_disable_focus(window: &mut Window, cx: &mut App) {
    // expect: disallowed_methods gpui::Window::disable_focus
    window.disable_focus(cx);
}

fn handle_focus(handle: &FocusHandle, window: &mut Window, cx: &mut App) {
    // expect: disallowed_methods gpui::FocusHandle::focus
    handle.focus(window, cx);
}

fn context_focus_view<T: 'static, W: Focusable>(
    cx: &mut Context<T>,
    view: &Entity<W>,
    window: &mut Window,
) {
    // expect: disallowed_methods gpui::Context::focus_view
    cx.focus_view(view, window);
}

// Overlays.

fn deferred() {
    // expect: disallowed_methods gpui::deferred
    let _ = gpui::deferred(div());
}

fn anchored() {
    // expect: disallowed_methods gpui::anchored
    let _ = gpui::anchored();
}

fn open_dialog(window: &mut Window, cx: &mut App) {
    // expect: disallowed_methods gpui_component::WindowExt::open_dialog
    window.open_dialog(cx, |dialog, _, _| dialog);
}

fn open_alert_dialog(window: &mut Window, cx: &mut App) {
    // expect: disallowed_methods gpui_component::WindowExt::open_alert_dialog
    window.open_alert_dialog(cx, |dialog, _, _| dialog);
}

fn open_sheet(window: &mut Window, cx: &mut App) {
    // expect: disallowed_methods gpui_component::WindowExt::open_sheet
    window.open_sheet(cx, |sheet, _, _| sheet);
}

fn open_sheet_at(window: &mut Window, cx: &mut App) {
    // expect: disallowed_methods gpui_component::WindowExt::open_sheet_at
    window.open_sheet_at(Placement::Left, cx, |sheet, _, _| sheet);
}

fn push_notification(window: &mut Window, cx: &mut App) {
    // expect: disallowed_methods gpui_component::WindowExt::push_notification
    window.push_notification("note", cx);
}

fn context_menu() {
    // expect: disallowed_methods gpui_component::menu::ContextMenuExt::context_menu
    let _ = div().id("demo").context_menu(|menu, _, _| menu);
}

fn dropdown_menu() {
    // expect: disallowed_methods gpui_component::menu::DropdownMenu::dropdown_menu
    let _ = Button::new("demo").dropdown_menu(|menu, _, _| menu);
}

fn dropdown_menu_with_anchor() {
    // expect: disallowed_methods gpui_component::menu::DropdownMenu::dropdown_menu_with_anchor
    let _ = Button::new("demo").dropdown_menu_with_anchor(gpui::Anchor::TopLeft, |menu, _, _| menu);
}

fn button_tooltip() {
    // expect: disallowed_methods gpui_component::button::Button::tooltip
    let _ = Button::new("demo").tooltip("hint");
}

fn button_tooltip_with_action(action: &dyn gpui::Action) {
    // expect: disallowed_methods gpui_component::button::Button::tooltip_with_action
    let _ = Button::new("demo").tooltip_with_action("hint", action, None);
}

fn toggle_tooltip() {
    // expect: disallowed_methods gpui_component::button::Toggle::tooltip
    let _ = Toggle::new("demo").tooltip("hint");
}

fn checkbox_tooltip() {
    // expect: disallowed_methods gpui_component::checkbox::Checkbox::tooltip
    let _ = gpui_component::checkbox::Checkbox::new("demo").tooltip("hint");
}

fn radio_tooltip() {
    // expect: disallowed_methods gpui_component::radio::Radio::tooltip
    let _ = gpui_component::radio::Radio::new("demo").tooltip("hint");
}

fn switch_tooltip() {
    // expect: disallowed_methods gpui_component::switch::Switch::tooltip
    let _ = gpui_component::switch::Switch::new("demo").tooltip("hint");
}

fn clipboard_tooltip() {
    // expect: disallowed_methods gpui_component::clipboard::Clipboard::tooltip
    let _ = gpui_component::clipboard::Clipboard::new("demo").tooltip("hint");
}

fn input_group_button_tooltip() {
    // expect: disallowed_methods gpui_component::input::InputGroupButton::tooltip
    let _ = gpui_component::input::InputGroupButton::new("demo").tooltip("hint");
}

// Detached work.

fn task_detach(task: Task<()>) {
    // expect: disallowed_methods gpui::Task::detach
    task.detach();
}

fn task_detach_and_log_err(task: Task<Result<(), String>>, cx: &App) {
    // expect: disallowed_methods gpui::TaskExt::detach_and_log_err
    task.detach_and_log_err(cx);
}

fn task_detach_and_log_err_with_backtrace(task: Task<Result<(), String>>, cx: &App) {
    // expect: disallowed_methods gpui::TaskExt::detach_and_log_err_with_backtrace
    task.detach_and_log_err_with_backtrace(cx);
}

// Threads, sleeps and clocks.

fn thread_spawn() {
    // expect: disallowed_methods std::thread::spawn
    std::thread::spawn(|| {});
}

fn thread_builder_spawn() {
    // expect: disallowed_methods std::thread::Builder::spawn
    let _ = std::thread::Builder::new().spawn(|| {});
}

fn thread_sleep() {
    // expect: disallowed_methods std::thread::sleep
    std::thread::sleep(Duration::ZERO);
}

fn instant_now() {
    // expect: disallowed_methods std::time::Instant::now
    let _ = std::time::Instant::now();
}

fn system_time_now() {
    // expect: disallowed_methods std::time::SystemTime::now
    let _ = std::time::SystemTime::now();
}

// Animation time.

fn executor_timer(executor: &BackgroundExecutor) {
    // expect: disallowed_methods gpui::BackgroundExecutor::timer
    let _ = executor.timer(Duration::ZERO);
}

fn window_request_animation_frame(window: &Window) {
    // expect: disallowed_methods gpui::Window::request_animation_frame
    window.request_animation_frame();
}

fn window_on_next_frame(window: &Window) {
    // expect: disallowed_methods gpui::Window::on_next_frame
    window.on_next_frame(|_, _| {});
}

fn context_on_next_frame<T: 'static>(cx: &Context<T>, window: &mut Window) {
    // expect: disallowed_methods gpui::Context::on_next_frame
    cx.on_next_frame(window, |_, _, _| {});
}

fn async_window_on_next_frame(cx: &mut AsyncWindowContext) {
    // expect: disallowed_methods gpui::AsyncWindowContext::on_next_frame
    cx.on_next_frame(|_, _| {});
}

fn with_animation() -> impl IntoElement {
    use gpui::AnimationExt;
    // expect: disallowed_methods gpui::AnimationExt::with_animation
    div().with_animation(
        "demo",
        gpui::Animation::new(Duration::ZERO),
        |element, _| element,
    )
}

fn with_animations() -> impl IntoElement {
    use gpui::AnimationExt;
    // expect: disallowed_methods gpui::AnimationExt::with_animations
    div().with_animations(
        "demo",
        vec![gpui::Animation::new(Duration::ZERO)],
        |element, _, _| element,
    )
}

// Raw colors.

fn color_rgb() {
    // expect: disallowed_methods gpui::rgb
    let _ = gpui::rgb(0);
}

fn color_rgba() {
    // expect: disallowed_methods gpui::rgba
    let _ = gpui::rgba(0);
}

fn color_hsla() {
    // expect: disallowed_methods gpui::hsla
    let _ = gpui::hsla(0.0, 0.0, 0.0, 0.0);
}

fn color_black() {
    // expect: disallowed_methods gpui::black
    let _ = gpui::black();
}

fn color_white() {
    // expect: disallowed_methods gpui::white
    let _ = gpui::white();
}

fn color_red() {
    // expect: disallowed_methods gpui::red
    let _ = gpui::red();
}

fn color_green() {
    // expect: disallowed_methods gpui::green
    let _ = gpui::green();
}

fn color_blue() {
    // expect: disallowed_methods gpui::blue
    let _ = gpui::blue();
}

fn color_yellow() {
    // expect: disallowed_methods gpui::yellow
    let _ = gpui::yellow();
}

fn color_transparent_black() {
    // expect: disallowed_methods gpui::transparent_black
    let _ = gpui::transparent_black();
}

fn color_transparent_white() {
    // expect: disallowed_methods gpui::transparent_white
    let _ = gpui::transparent_white();
}

fn color_opaque_grey() {
    // expect: disallowed_methods gpui::opaque_grey
    let _ = gpui::opaque_grey(0.0, 0.0);
}

// Kit and Base overlay types.

fn kit_dialog(_: gpui_component::dialog::Dialog) {
    // expect: disallowed_types gpui_component::dialog::Dialog
}

fn kit_popover(_: gpui_component::popover::Popover) {
    // expect: disallowed_types gpui_component::popover::Popover
}

fn kit_context_menu(_: gpui_component::menu::ContextMenu<gpui::Div>) {
    // expect: disallowed_types gpui_component::menu::ContextMenu
}

fn kit_popup_menu(_: gpui_component::menu::PopupMenu) {
    // expect: disallowed_types gpui_component::menu::PopupMenu
}

fn kit_notification(_: gpui_component::notification::Notification) {
    // expect: disallowed_types gpui_component::notification::Notification
}

fn kit_tooltip(_: gpui_component::tooltip::Tooltip) {
    // expect: disallowed_types gpui_component::tooltip::Tooltip
}

fn kit_sheet(_: gpui_component::sheet::Sheet) {
    // expect: disallowed_types gpui_component::sheet::Sheet
}

fn kit_hover_card(_: gpui_component::hover_card::HoverCard) {
    // expect: disallowed_types gpui_component::hover_card::HoverCard
}

fn kit_select(_: gpui_component::select::Select<gpui_component::select::SearchableVec<String>>) {
    // expect: disallowed_types gpui_component::select::Select
}

fn kit_combobox(
    _: gpui_component::combobox::Combobox<gpui_component::select::SearchableVec<String>>,
) {
    // expect: disallowed_types gpui_component::combobox::Combobox
}

fn kit_date_picker(_: gpui_component::date_picker::DatePicker) {
    // expect: disallowed_types gpui_component::date_picker::DatePicker
}

fn base_dialog(_: gpui_base::Dialog) {
    // expect: disallowed_types gpui_base::Dialog
}

fn base_alert_dialog(_: gpui_base::AlertDialog) {
    // expect: disallowed_types gpui_base::AlertDialog
}

fn base_popover(_: gpui_base::Popover) {
    // expect: disallowed_types gpui_base::Popover
}

fn base_deferred_popover(_: gpui_base::DeferredPopover) {
    // expect: disallowed_types gpui_base::DeferredPopover
}

fn base_hover_card(_: gpui_base::HoverCard) {
    // expect: disallowed_types gpui_base::HoverCard
}

fn base_tooltip(_: gpui_base::Tooltip) {
    // expect: disallowed_types gpui_base::Tooltip
}

fn base_tooltip_overlay(_: gpui_base::TooltipOverlay) {
    // expect: disallowed_types gpui_base::TooltipOverlay
}

fn base_select(_: gpui_base::Select) {
    // expect: disallowed_types gpui_base::Select
}

fn base_combobox(_: gpui_base::Combobox) {
    // expect: disallowed_types gpui_base::Combobox
}

fn base_date_picker(_: gpui_base::DatePicker) {
    // expect: disallowed_types gpui_base::DatePicker
}

// Added after the first review: more focus, tooltip, question, timing, animation, color and overlay paths.

fn context_focus_self<T: Focusable>(cx: &mut Context<T>, window: &mut Window) {
    // expect: disallowed_methods gpui::Context::focus_self
    cx.focus_self(window);
}

fn gpui_tooltip() {
    use gpui::StatefulInteractiveElement;
    // expect: disallowed_methods gpui::StatefulInteractiveElement::tooltip
    let _ = div().id("demo").tooltip(|_, _| unreachable!());
}

fn gpui_hoverable_tooltip() {
    use gpui::StatefulInteractiveElement;
    // expect: disallowed_methods gpui::StatefulInteractiveElement::hoverable_tooltip
    let _ = div().id("demo").hoverable_tooltip(|_, _| unreachable!());
}

fn attachment_tooltip(attachment: gpui_component::attachment::Attachment) {
    // expect: disallowed_methods gpui_component::attachment::Attachment::tooltip
    let _ = attachment.tooltip("hint");
}

fn window_prompt(window: &mut Window, cx: &mut App) {
    // expect: disallowed_methods gpui::Window::prompt
    let _ = window.prompt(gpui::PromptLevel::Info, "question", None, &["Yes"], cx);
}

fn async_window_prompt(cx: &mut AsyncWindowContext) {
    // expect: disallowed_methods gpui::AsyncWindowContext::prompt
    let _ = cx.prompt(gpui::PromptLevel::Info, "question", None, &["Yes"]);
}

fn thread_scope() {
    // expect: disallowed_methods std::thread::scope
    std::thread::scope(|_| {});
}

fn instant_elapsed(instant: std::time::Instant) {
    // expect: disallowed_methods std::time::Instant::elapsed
    let _ = instant.elapsed();
}

fn system_time_elapsed(time: std::time::SystemTime) {
    // expect: disallowed_methods std::time::SystemTime::elapsed
    let _ = time.elapsed();
}

fn tokio_task_spawn() {
    // expect: disallowed_methods tokio::task::spawn
    let _ = tokio::task::spawn(async {});
}

fn tokio_spawn_blocking() {
    // expect: disallowed_methods tokio::task::spawn_blocking
    let _ = tokio::task::spawn_blocking(|| {});
}

fn base_transition<T: gpui_base::Interpolate + PartialEq + 'static>(
    target: T,
    policy: gpui_base::Transition,
    window: &mut Window,
    cx: &mut App,
) -> T {
    // expect: disallowed_methods gpui_base::transition
    gpui_base::transition("demo", target, policy, window, cx)
}

fn base_spring<T: gpui::SpringTarget>(
    target: T,
    policy: gpui_base::Spring,
    window: &mut Window,
    cx: &mut App,
) -> T::Output {
    // expect: disallowed_methods gpui_base::spring
    gpui_base::spring("demo", target, policy, window, cx)
}

fn base_animate_keyframes<T: gpui_base::Interpolate + 'static>(
    keyframes: &gpui_base::Keyframes<T>,
    timing: gpui_base::Timing,
    window: &mut Window,
    cx: &mut App,
) {
    // expect: disallowed_methods gpui_base::animate_keyframes
    let _ = gpui_base::animate_keyframes("demo", keyframes, timing, window, cx);
}

fn hsla_red() {
    // expect: disallowed_methods gpui::Hsla::red
    let _ = gpui::Hsla::red();
}

fn hsla_green() {
    // expect: disallowed_methods gpui::Hsla::green
    let _ = gpui::Hsla::green();
}

fn hsla_blue() {
    // expect: disallowed_methods gpui::Hsla::blue
    let _ = gpui::Hsla::blue();
}

fn hsla_black() {
    // expect: disallowed_methods gpui::Hsla::black
    let _ = gpui::Hsla::black();
}

fn hsla_white() {
    // expect: disallowed_methods gpui::Hsla::white
    let _ = gpui::Hsla::white();
}

fn hsla_transparent_black() {
    // expect: disallowed_methods gpui::Hsla::transparent_black
    let _ = gpui::Hsla::transparent_black();
}

fn theme_hsl() {
    // expect: disallowed_methods gpui_component::theme::hsl
    let _ = gpui_component::theme::hsl(0.0, 0.0, 0.0);
}

fn theme_try_parse_color() {
    // expect: disallowed_methods gpui_component::theme::try_parse_color
    let _ = gpui_component::theme::try_parse_color("#000000");
}

fn theme_oklch() {
    // expect: disallowed_methods gpui_component::theme::oklch
    let _ = gpui_component::theme::oklch(0.0, 0.0, 0.0);
}

fn theme_black() {
    // expect: disallowed_methods gpui_component::theme::black
    let _ = gpui_component::theme::black();
}

fn theme_white() {
    // expect: disallowed_methods gpui_component::theme::white
    let _ = gpui_component::theme::white();
}

fn theme_try_parse_background() {
    // expect: disallowed_methods gpui_component::theme::try_parse_background
    let _ = gpui_component::theme::try_parse_background("#000000");
}

fn theme_color_name_scale() {
    // expect: disallowed_methods gpui_component::theme::ColorName::scale
    let _ = gpui_component::theme::ColorName::Red.scale(500);
}

fn theme_colorize_parse_hex() {
    // expect: disallowed_methods gpui_component::theme::Colorize::parse_hex
    let _ = <gpui::Hsla as gpui_component::theme::Colorize>::parse_hex("#000000");
}

fn base_popup(_: gpui_base::Popup) {
    // expect: disallowed_types gpui_base::Popup
}

fn base_toast(_: gpui_base::Toast) {
    // expect: disallowed_types gpui_base::Toast
}

fn base_toast_stack(_: gpui_base::ToastStack) {
    // expect: disallowed_types gpui_base::ToastStack
}

fn base_toast_manager(_: &gpui_base::ToastManager<u8, u8>) {
    // expect: disallowed_types gpui_base::ToastManager
}

// Panics that would abort inside an AppKit callback.

fn option_unwrap(value: Option<u8>) -> u8 {
    // expect: unwrap_used
    value.unwrap()
}

fn option_expect(value: Option<u8>) -> u8 {
    // expect: expect_used
    value.expect("demo")
}
