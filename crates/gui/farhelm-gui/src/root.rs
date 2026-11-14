//! A root view's wiring: the one way a window's root view takes on the layer manager and the focus coordinator.
//!
//! Every window's root view needs the same three things, and forgetting any of them fails quietly: a window without a
//! layer stack has no overlays, one whose root is not drawn through [`focus::root`] has no focus actions and no key
//! context for Farhelm's bindings, and one that draws [`layers::overlay`] outside that root leaves its dialogs and
//! menus outside Farhelm's key context, so Escape and Tab inside them reach nothing: GPUI draws every layer deferred,
//! above the whole tree whatever its place in it, but dispatches a deferred element's keys from the node it was
//! deferred under. There are several root views (the main view, the test harness's scene, the screenshot
//! goldens' scene), and a new one is usually written by copying another, so the rule lives here rather than in each:
//! a root view keeps one [`Attachment`] from [`attach`] and draws through [`wrap`].
//!
//! This module does not lay anything out; size, background and font stay with the root view that knows its window.

use gpui::{App, Context, Div, IntoElement, ParentElement as _, Window};

use crate::{focus, layers};

/// What a root view keeps for its window's overlays and focus: the window's layer stack and its focus state. Dropping
/// it detaches both from the window.
pub struct Attachment {
    // Layers first, both when attaching (in `attach`) and when dropping (fields drop in declaration order): the order
    // the root views moved onto this type used when they held the two attachments themselves. Nothing is known to
    // depend on it; it is kept so that this type changes nothing.
    _layers: layers::Attachment,
    focus: focus::Attachment,
}

/// Attach the layer manager and the focus coordinator to the root view's window. Every root view calls this once, from
/// its constructor, and draws through [`wrap`].
pub fn attach<V: 'static>(window: &mut Window, cx: &mut Context<V>) -> Attachment {
    Attachment {
        _layers: layers::attach(window, cx),
        focus: focus::attach(window, cx),
    }
}

/// The root view's outermost element: `element` (already styled, with the window's content as its children) wired as
/// the focus root, with the overlay inside it as a direct child.
///
/// Inside is what matters. The overlay's layers dispatch keys from where they are placed, so only there do dialogs and
/// menus get Farhelm's key context and the root's Escape and Tab handlers; and the overlay fills its parent, so its
/// parent must be the full-window element for a dialog's scrim to cover the window. The overlay goes last by
/// convention only: every layer is deferred, so where it sits among the content does not change what draws on top. The
/// result is opaque so that a root view hands its content in rather than arranging the two itself.
pub fn wrap(
    element: Div,
    attachment: &Attachment,
    window: &Window,
    cx: &App,
) -> impl IntoElement + use<> {
    focus::root(element, &attachment.focus).children(layers::overlay(window, cx))
}
