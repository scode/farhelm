//! Farhelm's GPUI desktop GUI.
//!
//! NOTE: This crate is a placeholder. It holds no code yet; it exists so the workspace pins GPUI and GPUI Kit (and
//! routes GPUI through the vendored, patched trees in `vendor/`) before the GUI itself lands.
//!
//! It will hold the GUI's views and the app-wide foundations they share: the store fed by the UI channel, focus,
//! overlays, prompts, operations, drafts, host-supplied text, the component layer, actions and keys, the animation
//! ticker and the notice center. SPEC_impl.md, "GUI architecture", is the rulebook for all of it, including the rule
//! that this crate never depends on the helm crate: everything it knows about the helm arrives over the helm's API.
