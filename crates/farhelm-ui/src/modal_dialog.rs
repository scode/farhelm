//! The modal boundary owns focus installation and release-before-return.
//!
//! A caller still owns its backdrop, contents and operation state. The shell
//! renders the existing dialog root itself, so extracting focus behavior does
//! not add a wrapper or change selectors. This is also the seam where a later
//! native `<dialog>` conversion can replace the boundary without moving each
//! dialog's contents or request ownership.
//! Every root shares one sequential Tab boundary and page isolation; callers
//! choose their initial handoff and optional gap behavior independently.
//! Scroll bodies use that same native entry rule, with node-local observation
//! and lost-focus repair that yields to a usable current owner.
//!
//! Closing is explicit: the caller decides whether its live operation permits
//! dismissal and which focus destination belongs to that close reason. DOM
//! removal only releases leftover isolation; it never returns focus, since a
//! successful handoff can intentionally give the keyboard to another surface.

use dioxus::prelude::*;

/// Address the same modal from its shell and from its surviving state owner.
///
/// Async success can close a dialog from its parent, so release cannot depend
/// on a child hook remaining mounted. A static selector may carry a numeric
/// opening fence; it still resolves the first root before checking identity.
/// This handle does not arbitrate multiple dialogs or queue them.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct DialogHandle {
    selector: &'static str,
    generation: Option<(&'static str, u64)>,
}

impl DialogHandle {
    /// Bind focus operations to the root whose lifecycle the caller owns.
    pub(crate) const fn new(selector: &'static str) -> Self {
        Self {
            selector,
            generation: None,
        }
    }

    /// Fence deferred DOM work against a later opening at the same selector.
    ///
    /// Resolve the first root before comparing its numeric generation. Filtering
    /// the selector instead could skip a replacement root and find an older one.
    pub(crate) const fn for_generation(mut self, attribute: &'static str, generation: u64) -> Self {
        self.generation = Some((attribute, generation));
        self
    }

    /// Compare only the already resolved root, before touching its focus state.
    fn generation_guard(self) -> String {
        self.generation
            .map_or(String::new(), |(attribute, generation)| {
                let attribute =
                    serde_json::to_string(attribute).expect("a string always serializes");
                format!(" || dialog.getAttribute({attribute}) !== '{generation}'")
            })
    }

    /// Apply an existing modal operation only to the expected mounted opening.
    fn bound_script(self, body: &str) -> String {
        if self.generation.is_none() {
            return body.to_owned();
        }
        let selector = serde_json::to_string(self.selector).expect("a string always serializes");
        let generation_guard = self.generation_guard();
        format!(
            "(() => {{ const dialog = document.querySelector({selector}); if (!dialog{generation_guard}) return; {body} }})();"
        )
    }

    /// Release only this opening; an absent or replacement root owns no release.
    fn release_script(self) -> String {
        self.bound_script(&crate::modal_isolation::release_js(self.selector))
    }

    /// Release isolation now and return focus after the closing render.
    ///
    /// The supplied body chooses the destination for this close reason. It
    /// may also resume a still-mounted parent's isolation around that action.
    /// Release and frame scheduling live here so no caller can focus its
    /// still-inert opener first. A missing destination retains the caller's
    /// existing no-op behavior.
    pub(crate) fn close(self, return_focus_body: &str) {
        document::eval(&format!(
            "{} requestAnimationFrame(() => {{ {return_focus_body} }});",
            self.release_script(),
        ));
    }

    /// Release for a modal handoff without scheduling a competing focus return.
    ///
    /// The next modal owns its initial focus. Submit this before hiding or
    /// clearing the root even when its close owes no return; DOM-removal
    /// cleanup remains the fallback for unmounts the owner does not initiate.
    pub(crate) fn release(self) {
        document::eval(&self.release_script());
    }

    /// Release only the modal rendered inside an existing domain owner.
    ///
    /// A delayed host question can retire before admission while another host's
    /// dialog is visible. Resolve its mounted owner first, then this root inside
    /// it, so that retirement cannot release the unrelated visible modal. A
    /// missing owner or root does nothing, and no focus return is scheduled.
    /// The same optional opening fence applies after the owned root is found.
    pub(crate) fn release_within(self, owner_selector: &str) {
        let owner = serde_json::to_string(owner_selector).expect("a string always serializes");
        let selector = serde_json::to_string(self.selector).expect("a string always serializes");
        let generation_guard = self.generation_guard();
        document::eval(&format!(
            r#"(() => {{
                const owner = document.querySelector({owner});
                const dialog = owner?.querySelector({selector});
                if (!dialog{generation_guard}) return;
                dialog.__farhelmModalRelease?.();
            }})();"#,
        ));
    }

    /// Return immediately after removal, unless a newer control already has focus.
    ///
    /// The surviving owner decides whether this return is owed and submits it
    /// from its existing post-render effect. A missing old root must not suppress
    /// that destination; generation fences only the release. Unlike `close`, no
    /// frame is scheduled. The body chooses only the domain destination.
    /// An isolated caller must still release before requesting removal, even
    /// when no return is owed; this post-render release cannot supply that order.
    pub(crate) fn close_immediately_if_document_focused(self, return_focus_body: &str) {
        document::eval(&format!(
            r#"{} (() => {{
                const active = document.activeElement;
                if (active !== document.body && active !== document.documentElement) return;
                {return_focus_body}
            }})();"#,
            self.release_script(),
        ));
    }

    /// Release isolation and wait for this captured root to leave the document.
    ///
    /// A terminal's focus veto also checks modal DOM, so releasing inert alone
    /// cannot complete a navigation handoff. The surviving owner must await
    /// this operation from its own task after requesting the closing render;
    /// a dialog-owned task would be cancelled by the removal it waits for.
    /// It must also submit `release` before that render: this async body does
    /// not run until polled, and its idempotent release may arrive after removal.
    /// Release and node capture share one eval, and removal is observed on
    /// that exact node rather than inferred from a frame or a later selector.
    /// As with the existing handoff, an eval failure does not block the owner.
    pub(crate) async fn close_and_wait(self) {
        let selector = serde_json::to_string(self.selector).expect("a string always serializes");
        let generation_match = self
            .generation
            .map_or(String::new(), |(attribute, generation)| {
                let attribute =
                    serde_json::to_string(attribute).expect("a string always serializes");
                format!(" && dialog.getAttribute({attribute}) === '{generation}'")
            });
        let js = format!(
            r#"{}
        const dialog = document.querySelector({selector});
        if (dialog{generation_match}) await new Promise(resolve => {{
            const observer = new MutationObserver(() => {{
                if (!dialog.isConnected) {{ observer.disconnect(); resolve(); }}
            }});
            observer.observe(document.body, {{ childList: true, subtree: true }});
        }});
        return true;"#,
            self.release_script()
        );
        let _ = document::eval(&js).join::<bool>().await;
    }

    /// Resume a mounted parent's isolation without repeating its initial focus.
    ///
    /// Nested help and consent return to parents whose gap handlers and focus
    /// memory remain installed. Their callers place this generated operation
    /// before or after the destination action in the shared close frame,
    /// preserving each handoff's existing order without submitting another eval.
    pub(crate) fn isolation_script(self, escape_target: Option<&str>) -> String {
        self.bound_script(&crate::modal_isolation::install_js(
            self.selector,
            escape_target,
        ))
    }

    /// Focus a revealed modal's surface at an explicit interaction boundary.
    ///
    /// Ordinary interactions resolve the root immediately. A reveal can precede
    /// its DOM update, so its caller requests one frame before lookup instead.
    /// Neither path reinstalls focus behavior or runs on ordinary rerenders.
    pub(crate) fn focus_surface(self, target: &FocusTarget, defer_for_reveal: bool) {
        let focus = self.surface_script(target);
        document::eval(&if defer_for_reveal {
            format!("requestAnimationFrame(() => {{ {focus} }})")
        } else {
            focus
        });
    }

    /// Resolve this root when the action runs, keeping reveal lookup deferred.
    fn surface_script(self, target: &FocusTarget) -> String {
        let selector = serde_json::to_string(self.selector).expect("a string always serializes");
        let generation_guard = self.generation_guard();
        let focus = target.surface_script();
        format!(
            "(() => {{ const dialog = document.querySelector({selector}); if (!dialog{generation_guard} || !dialog.getClientRects().length) return; {focus} }})();"
        )
    }

    /// Install the opening's focus policy without repeating it on rerenders.
    fn install(self, focus: &DialogFocus) {
        install_focus(self, focus);
    }
}

/// An opener captured before a modal takes focus, consumed after admission.
///
/// The caller owns capture and the decision whether this close owes a return.
/// A successful navigation can give focus to a new terminal or another modal;
/// spending the old capture still matters when no restoration is owed.
#[derive(Clone, Copy)]
pub(crate) struct SavedFocus {
    window_key: &'static str,
}

impl SavedFocus {
    /// Address the existing window slot without capturing or moving focus.
    pub(crate) const fn new(window_key: &'static str) -> Self {
        Self { window_key }
    }

    /// Consume the opener now and focus it only when this close owes a return.
    ///
    /// Call after isolation release and the caller's navigation decision.
    /// Consumption is unconditional, including a detached or unused opener;
    /// missing destinations have no fallback and no delayed return is queued.
    pub(crate) fn restore(self, restore: bool) {
        let key = serde_json::to_string(self.window_key).expect("a string always serializes");
        document::eval(&format!(
            r#"
        const previous = window[{key}];
        delete window[{key}];
        if ({restore} && previous?.isConnected) previous.focus({{ preventScroll: true }});
    "#
        ));
    }
}

/// Select a domain focus destination without owning modal release or timing.
///
/// Explicit handoffs resolve both selectors from document in priority order;
/// the installer uses the same rule relative to its mounted dialog. A missing
/// destination does nothing, preserving handoffs whose opener vanished. The
/// target can run after installation, on offer arrival, or as the destination-
/// only body of an already-approved close.
#[derive(Clone, PartialEq)]
pub(crate) struct FocusTarget {
    selector: String,
    fallback: Option<String>,
    quiet: bool,
}

impl FocusTarget {
    /// Describe the preferred target and optional fallback without moving focus.
    pub(crate) fn new(selector: impl Into<String>, fallback: Option<&str>, quiet: bool) -> Self {
        Self {
            selector: selector.into(),
            fallback: fallback.map(str::to_owned),
            quiet,
        }
    }

    /// Generate only the destination action; close owns release and scheduling.
    pub(crate) fn script(&self) -> String {
        target_js("document", self, false)
    }

    /// Perform a caller-owned handoff now, without reinstalling modal behavior.
    pub(crate) fn focus(&self) {
        document::eval(&self.script());
    }

    /// Focus descendants of an already captured, visible `dialog` root.
    ///
    /// The enclosing script supplies that exact node and checks its visibility.
    /// This lets a deferred repair keep its old-root connection and focus-
    /// ownership checks without resolving a later opening through the selector.
    /// A surface with neither descendant falls back to its root, including
    /// loading-only renders.
    pub(crate) fn surface_script(&self) -> String {
        target_js("dialog", self, true)
    }
}

/// Share target preference and quiet focus while retaining each lookup scope.
///
/// `scope` is an internal JS receiver, either the mounted dialog or document.
/// Remembered initial focus also falls back to its root; explicit handoffs do
/// not. The self-contained conditional never returns from an enclosing trap
/// installer when its initial target is absent, since the trap is still needed.
fn target_js(scope: &str, target: &FocusTarget, root_fallback: bool) -> String {
    let preferred = serde_json::to_string(&target.selector).expect("a string always serializes");
    let fallback = target.fallback.as_ref().map_or(String::new(), |selector| {
        let selector = serde_json::to_string(selector).expect("a string always serializes");
        format!(" ?? {scope}.querySelector({selector})")
    });
    let root = if root_fallback { " ?? dialog" } else { "" };
    let quiet = target.quiet;
    format!(
        r#"(() => {{
            const target = {scope}.querySelector({preferred}){fallback}{root};
            if (target) {{
                // Focus safety does not depend on the tooltip asset finishing load.
                // A later installation observes new events, not this past focus.
                if ({quiet} && window.farhelmTooltip?.focusWithoutTooltip) window.farhelmTooltip.focusWithoutTooltip(target);
                else target.focus({{ preventScroll: true }});
            }}
        }})();"#,
    )
}

/// Native usability is shared by Tab entry, scroll entry and lost-focus repair.
/// Sequential entry additionally requires nonnegative tabIndex; a deliberately
/// focused heading or approval answer can own focus with a negative tabIndex.
/// A managed scroll node whose stop was withdrawn must not become remembered
/// focus merely because an engine leaves it active after the attribute change.
const NATIVE_FOCUS_RULES: &str = r#"
    const nativeFocusSelector = 'button, input, textarea, select, a[href], summary, [tabindex]';
    const usableFocus = node => node instanceof HTMLElement && node.isConnected
        && !node.matches(':disabled') && !node.matches('input[type="hidden"]')
        && !node.closest('[hidden], [inert]') && node.getClientRects().length > 0
        && !['hidden', 'collapse'].includes(getComputedStyle(node).visibility);
    const sequentialFocus = (node, scope) => usableFocus(node)
        && scope.contains(node) && node.tabIndex >= 0;
    const chosenFocus = (node, dialog) => node !== document.body
        && node !== document.documentElement && node !== dialog && usableFocus(node)
        && !(node.__farhelmScrollRegion && !node.__farhelmScrollRegion.stop);
"#;

/// Existing response and write recovery retain their one- or two-frame priority.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum RecoveryDelay {
    OneFrame,
    TwoFrames,
}

/// A lost-focus destination, independent of request state and recovery triggers.
/// Callers keep their existing write/reset timing. Scroll entry uses the same
/// final owner check, so a later approval or programmatic heading keeps focus.
#[derive(Clone, PartialEq)]
pub(crate) struct RecoveryFocus {
    target: FocusTarget,
    remember: bool,
    delay: RecoveryDelay,
}

impl RecoveryFocus {
    /// Preserve domain target priority without accepting arbitrary recovery code.
    pub(crate) fn new(
        preferred: &str,
        fallback: Option<&str>,
        remember: bool,
        delay: RecoveryDelay,
    ) -> Self {
        Self {
            target: FocusTarget::new(preferred, fallback, false),
            remember,
            delay,
        }
    }

    /// Generate only the final action, with the exact captured `dialog` in scope.
    /// Domain wrappers run checkbox resets and their existing frame delay first.
    pub(crate) fn script(&self) -> String {
        let policy = self.policy();
        format!("(() => {{ {NATIVE_FOCUS_RULES} const recovery = {policy}; {RECOVER_FOCUS} }})();")
    }

    /// Serialize destination facts rather than transporting another state owner.
    fn policy(&self) -> String {
        serde_json::json!({
            "preferred": self.target.selector,
            "fallback": self.target.fallback,
            "remember": self.remember,
            "frames": if self.delay == RecoveryDelay::TwoFrames { 2 } else { 1 },
        })
        .to_string()
    }
}

/// Recheck the current owner at execution time, including exempt outside input.
/// A withdrawn managed stop is the exception to preserving an active -1 node.
const RECOVER_FOCUS: &str = r#"
    if (!usableFocus(dialog) || chosenFocus(document.activeElement, dialog)) return;
    const last = dialog.__farhelmLastFocus;
    const preferred = dialog.querySelector(recovery.preferred);
    const fallback = recovery.fallback ? dialog.querySelector(recovery.fallback) : null;
    const target = recovery.remember && dialog.contains(last) && chosenFocus(last, dialog)
        ? last : chosenFocus(preferred, dialog) ? preferred
        : chosenFocus(fallback, dialog) ? fallback : dialog;
    target.focus({ preventScroll: true });
"#;

/// Render the existing body div and own its conditional keyboard entry point.
/// The name is present only when no native sequential descendant supplies entry.
/// This does not measure overflow or create another scroll container. Local DOM
/// observation follows busy, loading and responsive changes without domain flags.
/// Callers supply an exact local selector for their existing body class; this
/// component owns tabindex, role and aria-label rather than forwarding copies.
#[component]
pub(crate) fn ScrollRegion(
    handle: DialogHandle,
    selector: &'static str,
    name: String,
    recovery: RecoveryFocus,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    // Feedback changes its meaningful name after sending. Reconfiguration only
    // updates node-local data; it cannot stack listeners or repeat opening focus.
    use_effect(use_reactive((&name, &recovery), move |(name, recovery)| {
        install_scroll_region(handle, Some(selector), Some(&name), &recovery);
    }));
    rsx! {
        div {
            onmounted: move |_| install_scroll_region(handle, Some(selector), Some(&name), &recovery),
            ..attributes,
            {children}
        }
    }
}

/// Bind one scroll owner to one captured modal, retaining native root semantics.
/// Body mode owns region attributes; root mode changes only tabindex, restoring
/// -1 when ordinary controls return. No delayed operation queries a replacement.
fn install_scroll_region(
    handle: DialogHandle,
    selector: Option<&str>,
    name: Option<&str>,
    recovery: &RecoveryFocus,
) {
    let dialog = serde_json::to_string(handle.selector).expect("a string always serializes");
    let selector = serde_json::to_string(&selector).expect("a selector always serializes");
    let name = serde_json::to_string(&name).expect("a name always serializes");
    let generation_guard = handle.generation_guard();
    let policy = recovery.policy();
    document::eval(&format!(
        r#"(() => {{
            const dialog = document.querySelector({dialog});
            if (!dialog{generation_guard}) return;
            const selector = {selector};
            const region = selector ? dialog.querySelector(selector) : dialog;
            if (!region) return;
            const config = {{ name: {name}, recovery: {policy} }};
            const installed = region.__farhelmScrollRegion;
            if (installed?.dialog === dialog) {{
                installed.config = config;
                installed.schedule();
                return;
            }}
            {NATIVE_FOCUS_RULES}
            // Ownership survives native blur to body, but an independent focus
            // choice retires it. No request or draft state is captured here.
            const state = {{ dialog, config, stop: false, owned: region.contains(document.activeElement), frame: null, recoveryFrame: null }};
            region.__farhelmScrollRegion = state;
            const attribute = (name, value) => {{
                if (value === null) {{ if (region.hasAttribute(name)) region.removeAttribute(name); }}
                else if (region.getAttribute(name) !== value) region.setAttribute(name, value);
            }};
            const recover = () => {{
                state.recoveryFrame = null;
                if (!region.isConnected || !usableFocus(region) || !usableFocus(dialog)
                    || chosenFocus(document.activeElement, dialog)) return;
                if (state.stop) {{
                    // Automatic parking must not replace the control whose
                    // loss caused recovery. Manual Tab into the region still
                    // records its ordinary focus choice through the listener.
                    const last = dialog.__farhelmLastFocus;
                    region.focus({{ preventScroll: true }});
                    dialog.__farhelmLastFocus = last;
                    return;
                }}
                const recovery = state.config.recovery;
                {RECOVER_FOCUS}
            }};
            const reconcile = initial => {{
                if (!region.isConnected || !dialog.isConnected) {{ dispose(); return; }}
                const active = document.activeElement;
                const wasStop = state.stop;
                state.stop = ![...region.querySelectorAll(nativeFocusSelector)]
                    .some(node => sequentialFocus(node, region));
                attribute('tabindex', state.stop ? '0' : selector ? null : '-1');
                if (selector) {{
                    attribute('role', state.stop ? 'region' : null);
                    attribute('aria-label', state.stop ? state.config.name : null);
                }}
                // Withdrawing text-only entry also repairs a scrim click's
                // document/root focus, even when this body never owned it.
                // Other off-focus attribute changes do not gain that claim.
                const withdrewFocus = wasStop && !state.stop
                    && (active === region || active === document.body
                        || active === document.documentElement || active === dialog);
                // Engines differ on whether disabling a control immediately
                // blurs it. Both forms of that same loss need native entry.
                const lost = state.owned && !chosenFocus(active, dialog)
                    && (active === document.body || active === document.documentElement
                        || active === dialog || region.contains(active));
                if (!initial && (withdrewFocus || lost) && usableFocus(region) && usableFocus(dialog)) {{
                    // Reconciliation already runs in frame one. SSH recovers
                    // here; the two-frame policies yield once more to navigation.
                    if (state.config.recovery.frames === 1) recover();
                    else if (state.recoveryFrame === null) state.recoveryFrame = requestAnimationFrame(recover);
                }}
            }};
            const schedule = () => {{
                if (state.frame === null) state.frame = requestAnimationFrame(() => {{ state.frame = null; reconcile(false); }});
            }};
            state.schedule = schedule;
            const focus = event => {{
                if (region.contains(event.target)) state.owned = true;
                // Isolation can park focus on the root after a loss; it is
                // not an independent control that retires this body's claim.
                else if (event.target !== dialog && event.target !== document.body
                    && event.target !== document.documentElement) state.owned = false;
            }};
            const observer = new MutationObserver(records => {{
                if (!region.isConnected || !dialog.isConnected) {{ dispose(); return; }}
                if (records.some(record => !(record.type === 'attributes'
                    && record.target === region && record.attributeName === 'tabindex'))) schedule();
            }});
            const resize = new ResizeObserver(schedule);
            const dispose = () => {{
                observer.disconnect(); resize.disconnect();
                document.removeEventListener('focusin', focus, true);
                window.removeEventListener('resize', schedule);
                if (state.frame !== null) cancelAnimationFrame(state.frame);
                if (state.recoveryFrame !== null) cancelAnimationFrame(state.recoveryFrame);
                if (region.__farhelmScrollRegion === state) delete region.__farhelmScrollRegion;
            }};
            observer.observe(region, {{ childList: true, subtree: true, attributes: true,
                attributeFilter: ['disabled', 'hidden', 'tabindex', 'class', 'style', 'inert', 'type', 'href', 'open'] }});
            // Direct-child removal still arrives after explicit close released
            // isolation. Hidden parents remain installed and merely reconcile.
            for (let parent = region.parentElement; parent; parent = parent.parentElement) {{
                observer.observe(parent, {{ childList: true, attributes: true,
                    attributeFilter: ['disabled', 'hidden', 'inert', 'class', 'style'] }});
            }}
            resize.observe(region);
            document.addEventListener('focusin', focus, true);
            window.addEventListener('resize', schedule);
            reconcile(true);
        }})();"#,
    ));
}

/// Give every modal the same sequential boundary without changing interior Tab.
///
/// Boundary wrapping deliberately uses native focus scrolling: a contained
/// control outside the dialog's scroll viewport must become visible too.
/// The opening's focus handoff shares the once-per-node guard, so a later child
/// question or an independent approval keeps its own focus on rerenders.
fn trap_js(handle: DialogHandle, initial_focus: &str) -> String {
    let dialog = serde_json::to_string(handle.selector).expect("a string always serializes");
    let generation_guard = handle.generation_guard();
    let listener = r#"
                // Every selector branch passes the same native eligibility test.
                // In particular, arrow-owned buttons with tabindex=-1 and controls
                // disabled by a fieldset must not become scripted Tab destinations.
                const eligible = node => sequentialFocus(node, dialog);
                const sameRadioGroup = (left, right) => left instanceof HTMLInputElement
                    && right instanceof HTMLInputElement && left.type === 'radio' && right.type === 'radio'
                    && left.name !== '' && left.name === right.name && left.form === right.form;
                dialog.addEventListener('keydown', event => {
                    if (event.key !== 'Tab') return;
                    const candidates = [...dialog.querySelectorAll(nativeFocusSelector)].filter(eligible);
                    // Radio groups are one native Tab stop. An unanswered group
                    // admits its first answer forward and its last answer backward;
                    // arrow navigation within it remains entirely browser-owned.
                    const nodes = candidates.filter(node => {
                        if (!(node instanceof HTMLInputElement) || node.type !== 'radio' || !node.name) return true;
                        const group = candidates.filter(other => sameRadioGroup(node, other));
                        const target = group.find(other => other.checked)
                            || (event.shiftKey ? group[group.length - 1] : group[0]);
                        return node === target;
                    });
                    if (!nodes.length) {
                        event.preventDefault();
                        dialog.focus({ preventScroll: true });
                        return;
                    }
                    const first = nodes[0], last = nodes[nodes.length - 1];
                    const active = document.activeElement;
                    const atStart = active === first || sameRadioGroup(active, first);
                    const atEnd = active === last || sameRadioGroup(active, last);
                    if (active === dialog || (event.shiftKey ? atStart : atEnd)) {
                        event.preventDefault();
                        (event.shiftKey ? last : first).focus();
                    }
                });"#;
    format!(
        r#"(() => {{
                const dialog = document.querySelector({dialog});
                if (!dialog{generation_guard} || dialog.__farhelmDialogTrap) return;
                dialog.__farhelmDialogTrap = true;
                {initial_focus}
                {NATIVE_FOCUS_RULES}
                {listener}
            }})();"#,
    )
}

/// Preserve an opening's admission decision and consume its exact saved opener.
///
/// Sample once and compare before clearing the capture, even when unconditional
/// naming focus wins or an unrelated control makes ordinary focus yield. The
/// caller places this body behind the generation and once-per-node guards.
fn handoff_js(
    selector: &str,
    select_all: bool,
    captured_opener: Option<&str>,
    admission: &FocusAdmission,
) -> String {
    let target = serde_json::to_string(selector).expect("a string always serializes");
    let capture = captured_opener.map_or_else(
        || "const capturedOpener = false;".to_owned(),
        |key| {
            let key = serde_json::to_string(key).expect("a string always serializes");
            format!("const capturedOpener = active === window[{key}]; window[{key}] = null;")
        },
    );
    let admitted = match admission {
        FocusAdmission::Always => "true".to_owned(),
        FocusAdmission::DocumentOrOpener { selector } => {
            let selector = serde_json::to_string(selector).expect("a string always serializes");
            format!(
                "!active || active === document.body || active.matches({selector}) || capturedOpener"
            )
        }
    };
    let select = if select_all { "input?.select();" } else { "" };
    format!(
        r#"const active = document.activeElement;
                {capture}
                if ({admitted}) {{
                    const input = dialog.querySelector({target});
                    input?.focus({{ preventScroll: true }});
                    {select}
                }}"#,
    )
}

/// Install the shell's opening policy in its existing eval order.
///
/// Initial focus precedes isolation, which otherwise blurs an inerted opener.
/// Remembered dialogs keep their existing two eval submissions: focus and
/// isolation first, gap/Enter second. Direct trapped dialogs keep gap/Enter
/// before isolation in one eval. These are mount contracts, not effects that
/// should run again whenever a dialog's state changes. A preparable surface
/// submits gap/Enter and trap first, then samples its live hidden state: hidden
/// preparation isolates without focus, while reveal owns a later handoff.
/// Visible surfaces likewise isolate, after their existing focus submission.
fn install_focus(handle: DialogHandle, focus: &DialogFocus) {
    let dialog_selector = handle.selector;
    let dialog = serde_json::to_string(dialog_selector).expect("a string always serializes");
    let generation_guard = handle.generation_guard();
    let setup = match &focus.initial_focus {
        InitialFocus::Target(selector) => {
            let target = FocusTarget::new(selector.clone(), None, false);
            let initial = target_js("dialog", &target, false);
            trap_js(handle, &initial)
        }
        InitialFocus::Remembered { selector, quiet } => {
            // Disabled confirming answers retain a separate Cancel/root fallback;
            // the independent Escape selector still chooses its ordinary handler.
            let target = FocusTarget::new(
                selector.clone(),
                Some(".host-settings-close, .uninstall-cancel"),
                *quiet,
            );
            let initial_focus = target_js("dialog", &target, true);
            let mut setup = format!(
                r#"(() => {{
                    const dialog = document.querySelector({dialog});
                    if (!dialog{generation_guard}) return;
                    if (!dialog.__farhelmFocusMemory) {{
                        dialog.__farhelmFocusMemory = true;
                        dialog.addEventListener('focusin', (event) => {{
                            if (event.target !== dialog) dialog.__farhelmLastFocus = event.target;
                        }});
                    }}
                    {initial_focus}
                }})();"#,
            );
            setup.push_str(&trap_js(handle, ""));
            setup
        }
        InitialFocus::Handoff {
            selector,
            select_all,
            captured_opener,
            admission,
        } => trap_js(
            handle,
            &handoff_js(selector, *select_all, *captured_opener, admission),
        ),
        InitialFocus::Surface { .. } => trap_js(handle, ""),
    };
    let action = focus.main_action.as_deref().map_or(
        crate::dialog_focus::ContainerEnter::None,
        crate::dialog_focus::ContainerEnter::Click,
    );
    let gap = if focus.gap {
        handle.bound_script(&crate::dialog_focus::install_js(dialog_selector, action))
    } else {
        String::new()
    };
    let isolate = handle.isolation_script(focus.escape_target.as_deref());
    match &focus.initial_focus {
        InitialFocus::Target(_) | InitialFocus::Handoff { .. } => {
            document::eval(&format!("{setup}\n{gap}\n{isolate}"));
        }
        InitialFocus::Remembered { .. } => {
            document::eval(&format!("{setup} {isolate}"));
            document::eval(&gap);
        }
        InitialFocus::Surface { target, hidden_now } => {
            document::eval(&format!("{gap}\n{setup}"));
            // Preserve the live read after the first eval submission. Moving
            // it into render or page JS changes which opening state it sees.
            if hidden_now.call(()) {
                document::eval(&handle.isolation_script(focus.escape_target.as_deref()));
            } else {
                document::eval(&format!("{} {isolate}", handle.surface_script(target)));
            }
        }
    };
}

/// Preserve each dialog family's initial-focus and installation-order contract.
///
/// The remembered policy keeps the host fallback and recovery memory together;
/// the direct target retains Restart with's focus inside the trap's guard.
#[derive(Clone, PartialEq)]
pub(crate) enum InitialFocus {
    /// Focus only the named descendant, without fallback or focus memory.
    Target(String),
    /// Remember focus before moving it, with the host family's Cancel/root fallback.
    ///
    /// Memory installation is idempotent; initial focus still runs on every
    /// explicit installation, as it did for the host installers. Quiet focus
    /// suppresses the initial tooltip without depending on its asset loading.
    Remembered { selector: String, quiet: bool },
    /// Hand off once, yielding to newer focus unless the opening owns all sources.
    ///
    /// A supplied capture is compared and nulled even when focus is denied.
    /// An admitted naming handoff can select its title while ordinary editing
    /// leaves the native selection untouched.
    Handoff {
        selector: String,
        select_all: bool,
        captured_opener: Option<&'static str>,
        admission: FocusAdmission,
    },
    /// Prepare without focusing hidden controls; reveal is a separate action.
    ///
    /// The installer calls `hidden_now` once after submitting gap/Enter and
    /// trap setup. Hidden preparation isolates without focus; a visible opening
    /// isolates after its preferred/fallback/root handoff.
    /// The callback reads live caller state without making installation reactive.
    Surface {
        target: FocusTarget,
        hidden_now: Callback<(), bool>,
    },
}

/// Which current focus sources an opening is allowed to replace.
#[derive(Clone, PartialEq)]
pub(crate) enum FocusAdmission {
    /// The opening owns one handoff regardless of the current active control.
    Always,
    /// Yield to later outside focus, except for the caller's named or saved opener.
    ///
    /// Missing focus and body are admitted; documentElement is deliberately
    /// excluded here, unlike the post-removal return rule.
    DocumentOrOpener { selector: String },
}

/// Keep mount targets separate from live cancellation and request ownership.
///
/// Every modal traps sequential Tab and isolates the page. Initial focus never
/// implies that the same action handles Escape or container Enter. Gap-click
/// retention remains independent because Rename deliberately keeps native gaps.
#[derive(Clone, PartialEq)]
pub(crate) struct DialogFocus {
    pub(crate) initial_focus: InitialFocus,
    pub(crate) escape_target: Option<String>,
    pub(crate) main_action: Option<String>,
    /// Preserve gap-click retention independently of a container Enter action.
    pub(crate) gap: bool,
}

impl DialogFocus {
    /// Keep the host family's focus memory and quiet initial-target policy.
    ///
    /// Cancellation and container Enter retain independent live selectors;
    /// neither changes the shared trap or isolation contract.
    pub(crate) fn remembered(
        initial_focus: &str,
        escape_target: Option<&str>,
        quiet: bool,
        main_action: Option<&str>,
    ) -> Self {
        Self {
            initial_focus: InitialFocus::Remembered {
                selector: initial_focus.to_owned(),
                quiet,
            },
            escape_target: escape_target.map(str::to_owned),
            main_action: main_action.map(str::to_owned),
            gap: true,
        }
    }
}

/// Keep a form dialog's native submit boundary on the modal root itself.
///
/// The shell adds no wrapper: controls and form submission retain the same
/// ancestor and CSS relationships as the original div or form root.
#[derive(Clone, Copy, Default, PartialEq)]
pub(crate) enum DialogRoot {
    /// Keep ordinary dialogs outside the browser's native submit behavior.
    #[default]
    Div,
    /// Let destination-field Enter and explicit submit buttons submit this root.
    Form,
}

/// Render one existing modal root and own its mount-time focus boundary.
///
/// Attributes remain caller-supplied so markup, labels and root focusability
/// stay unchanged. Escape delegates to the caller's live state and layered
/// cancellation rules; other keys keep their existing root handler. Child
/// focus handoffs remain independent, and ordinary rerenders never repeat
/// initial focus over a later child question. The optional mounted callback
/// runs after the shared installation has been submitted; it preserves domain
/// work such as Settings' delayed trust-section scroll, not a second installer.
/// Movement callbacks attach to this same root only when supplied, so dialogs
/// without resize behavior do not acquire an otherwise unused resize observer.
/// Click handling likewise stays on the root when a caller separates its
/// controls from a dismissible backdrop; absent handlers remain absent.
/// An optional second focus target preserves setup's existing root-first
/// isolation, then quiet confirmation handoff, before domain mounted work.
/// A nested consent may first release its still-mounted parent; this remains
/// a separate eval before initial focus, rather than another mount callback.
/// Focus ownership listeners also attach to the actual root: a wrapper would
/// change the boundary that records whether an async close still owns focus.
/// Form-only validity suppression remains explicit rather than acquiring
/// browser validation when its root moves behind this component boundary.
/// Root scroll entry changes only tabindex and retains the dialog's role/name;
/// its -1 baseline stays out of sequential navigation when controls are usable.
#[component]
pub(crate) fn ModalDialog(
    handle: DialogHandle,
    focus: DialogFocus,
    on_escape: EventHandler<KeyboardEvent>,
    #[props(default)] root: DialogRoot,
    #[props(default)] novalidate: Option<bool>,
    #[props(default)] root_scroll: Option<RecoveryFocus>,
    #[props(default)] before_install_release: Option<DialogHandle>,
    #[props(default)] after_install_focus: Option<FocusTarget>,
    #[props(default)] onkeydown: Option<EventHandler<KeyboardEvent>>,
    #[props(default)] onmounted: Option<EventHandler<MountedEvent>>,
    #[props(default)] onscroll: Option<EventHandler<ScrollEvent>>,
    #[props(default)] onresize: Option<EventHandler<ResizeEvent>>,
    #[props(default)] onsubmit: Option<EventHandler<FormEvent>>,
    #[props(default)] onclick: Option<EventHandler<MouseEvent>>,
    #[props(default)] oninput: Option<EventHandler<FormEvent>>,
    #[props(default)] onchange: Option<EventHandler<FormEvent>>,
    #[props(default)] onfocusin: Option<EventHandler<FocusEvent>>,
    #[props(default)] onfocusout: Option<EventHandler<FocusEvent>>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let scrolls_root = root_scroll.is_some();
    // Native roots share one lifecycle boundary. Keeping their handlers here
    // avoids giving forms a second copy of focus installation or Escape rules.
    let mut root_attributes = vec![
        dioxus::html::onmounted(move |event: MountedEvent| {
            if let Some(parent) = before_install_release {
                parent.release();
            }
            handle.install(&focus);
            if let Some(target) = &after_install_focus {
                target.focus();
            }
            if let Some(onmounted) = onmounted {
                onmounted.call(event);
            }
            if let Some(recovery) = &root_scroll {
                install_scroll_region(handle, None, None, recovery);
            }
        }),
        dioxus::html::onkeydown(move |event: KeyboardEvent| {
            if event.key() == Key::Escape {
                on_escape.call(event);
            } else if let Some(onkeydown) = onkeydown {
                onkeydown.call(event);
            }
        }),
    ];
    root_attributes.extend(attributes);
    if scrolls_root {
        root_attributes.push(Attribute::new("tabindex", "-1", None, false));
    }
    // GlobalAttributes forwards presentation attributes, but native events
    // need explicit props. Absent listeners remain absent.
    if let Some(onscroll) = onscroll {
        root_attributes.push(dioxus::html::onscroll(move |event: ScrollEvent| {
            onscroll.call(event);
        }));
    }
    if let Some(onresize) = onresize {
        root_attributes.push(dioxus::html::onresize(move |event: ResizeEvent| {
            onresize.call(event);
        }));
    }
    if let Some(onsubmit) = onsubmit {
        root_attributes.push(dioxus::html::onsubmit(move |event: FormEvent| {
            onsubmit.call(event);
        }));
    }
    if let Some(onclick) = onclick {
        root_attributes.push(dioxus::html::onclick(move |event: MouseEvent| {
            onclick.call(event);
        }));
    }
    if let Some(oninput) = oninput {
        root_attributes.push(dioxus::html::oninput(move |event: FormEvent| {
            oninput.call(event);
        }));
    }
    if let Some(onchange) = onchange {
        root_attributes.push(dioxus::html::onchange(move |event: FormEvent| {
            onchange.call(event);
        }));
    }
    if let Some(onfocusin) = onfocusin {
        root_attributes.push(dioxus::html::onfocusin(move |event: FocusEvent| {
            onfocusin.call(event);
        }));
    }
    if let Some(onfocusout) = onfocusout {
        root_attributes.push(dioxus::html::onfocusout(move |event: FocusEvent| {
            onfocusout.call(event);
        }));
    }
    match root {
        DialogRoot::Div => rsx! { div { ..root_attributes, {children} } },
        DialogRoot::Form => rsx! { form { novalidate, ..root_attributes, {children} } },
    }
}
