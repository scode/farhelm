//! Browser-free component tests: render the real UI headlessly, drive it
//! with simulated events, and read the result back as a DOM.
//!
//! The browser suite (`e2e/`) takes hours and runs late, so most breakage it
//! finds is found after the work is "done". This harness is the fast inner
//! layer for the part of that suite that never needed a browser: what the
//! page says, which controls exist and in what state, and which request the
//! UI sends after a click. A test here takes about a second (mounting the
//! whole app dominates) under the plain `cargo nextest run -p farhelm-ui`
//! selection that every change already runs.
//!
//! ## What it models, and what it does not
//!
//! - The DOM is real Dioxus output ([`dom`]): the same templates, attributes
//!   and text the browser would hold, across re-renders.
//! - Events go through the Dioxus runtime with Dioxus's serialized event
//!   data, so handlers, bubbling and `prevent_default` behave as in a page.
//!   A click on a submit button also submits its form, as a browser would.
//! - The API is a [`server::FakeHelm`] serving canned JSON over real HTTP.
//! - There is no layout, no CSS, no focus, and no page JavaScript:
//!   `document::eval` calls (focus moves, scrolling, measuring, the event
//!   feed's socket) find no renderer and do nothing. Anything that depends on
//!   them stays a browser test.
//! - The UI runs its native code paths. The wasm/native split lives in a few
//!   modules (`api`, `auth`, `reader`, `activity`, `approvals`, the crate
//!   root); a behavior that differs there is not proven here for the web
//!   build.
//!
//! ## Writing a test
//!
//! ```ignore
//! #[farhelm_testtrace::test]
//! async fn the_launcher_names_its_harness_choices() {
//!     let helm = FakeHelm::start().await;
//!     fixtures::idle_helm(&helm);                 // the reads every page makes
//!     let mut page = Page::app(&helm);            // the whole app, as a browser loads it
//!     let new = page.wait_for(".new-session-button").await;
//!     page.click(new);
//!     let form = page.wait_for(".create-session-form").await;
//!     let codex = page.role_in(form, "button", "Codex");
//!     assert_eq!(page.dom().attr(codex, "aria-pressed"), Some("false"));
//! }
//! ```
//!
//! Waits are event-driven: [`Page::wait_until`] re-checks its condition each
//! time the VirtualDom has work and fails at a deadline with the page
//! outline and the requests the fake helm saw, so a missing route or a
//! renamed control explains itself.

pub mod dom;
pub mod fixtures;
mod quiet;
pub mod server;

mod harness_tests;
mod hosts_tests;
mod launcher_tests;

use dioxus::core::{ElementId, Event};
use dioxus::html::{
    PlatformEventData, SerializedFormData, SerializedHtmlEventConverter, SerializedMouseData,
};
use dioxus::prelude::*;
use dom::{Dom, Kind, NodeId};
use server::FakeHelm;
use std::{rc::Rc, time::Duration};
use tracing::instrument::WithSubscriber;

/// How long a wait may take before the test fails. Generous on purpose:
/// everything here is in-process, so a wait that needs more than this is a
/// missing route or a broken condition, not a slow machine.
const WAIT: Duration = Duration::from_secs(10);

/// A mounted UI and the DOM it renders into.
pub struct Page {
    vdom: VirtualDom,
    dom: Dom,
    /// The fake helm's base, kept for failure messages.
    base: String,
    /// The fake's request log, read when a wait fails.
    requests: Box<dyn Fn() -> Vec<server::Recorded>>,
    /// The subtree failure messages show, when a test has narrowed to one
    /// surface. A whole-app outline is mostly sidebar.
    dump_root: Option<NodeId>,
    /// The capture minus Dioxus's TRACE noise; see [`quiet`].
    quiet: tracing::Dispatch,
    /// How long a wait may take; [`WAIT`] unless a test shortens it.
    wait: Duration,
}

impl Page {
    /// Mount the whole application body against `helm`, the way the web
    /// build mounts it under a browser (minus the window frame, which is
    /// desktop chrome and page script only).
    pub fn app(helm: &FakeHelm) -> Self {
        Self::mount(helm, crate::AppBody)
    }

    /// Mount any root component against `helm`.
    pub fn mount(helm: &FakeHelm, root: fn() -> Element) -> Self {
        // The serialized converter is what turns this harness's event data
        // into Dioxus's typed events; desktop installs the same one.
        dioxus::html::set_event_converter(Box::new(SerializedHtmlEventConverter));
        let mut vdom =
            VirtualDom::new(root).with_root_context(crate::ApiBase(helm.base().to_owned()));
        let mut dom = Dom::default();
        let quiet = quiet::Quiet::current();
        tracing::dispatcher::with_default(&quiet, || vdom.rebuild(&mut dom));
        let state = helm.requests_handle();
        Page {
            vdom,
            dom,
            base: helm.base().to_owned(),
            requests: Box::new(state),
            dump_root: None,
            quiet,
            wait: WAIT,
        }
    }

    /// Apply whatever the VirtualDom has queued to the DOM.
    fn render(&mut self) {
        let (vdom, dom) = (&mut self.vdom, &mut self.dom);
        tracing::dispatcher::with_default(&self.quiet, || vdom.render_immediate(dom));
    }

    /// The DOM as last rendered, for reading attributes and structure.
    pub fn dom(&self) -> &Dom {
        &self.dom
    }

    /// Change how long waits may take. Only for tests that expect a wait to
    /// fail, which otherwise spend the whole default deadline proving it.
    pub fn set_wait(&mut self, wait: Duration) {
        self.wait = wait;
    }

    /// Show only `node`'s subtree in failure messages from now on (while it
    /// stays attached), so a failing launcher test prints the launcher.
    pub fn dump_only(&mut self, node: NodeId) {
        self.dump_root = Some(node);
    }

    // --- waiting -----------------------------------------------------------

    /// Run the UI until `condition` holds, or fail at the deadline.
    ///
    /// The condition is re-checked after every render, and the VirtualDom
    /// is only awaited when it has nothing to do, so this returns as soon as
    /// the page reaches the state, without sleeping.
    pub async fn wait_until<T>(
        &mut self,
        what: &str,
        mut condition: impl FnMut(&Page) -> Option<T>,
    ) -> T {
        let deadline = tokio::time::Instant::now() + self.wait;
        loop {
            self.render();
            if let Some(found) = condition(self) {
                return found;
            }
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero()
                || tokio::time::timeout(
                    remaining,
                    self.vdom
                        .wait_for_work()
                        .with_subscriber(self.quiet.clone()),
                )
                .await
                .is_err()
            {
                self.render();
                if let Some(found) = condition(self) {
                    return found;
                }
                panic!("{}", self.failure(&format!("timed out waiting for {what}")));
            }
        }
    }

    /// Wait until `selector` matches exactly one attached node, and return it.
    pub async fn wait_for(&mut self, selector: &str) -> NodeId {
        let root = self.dom.root();
        self.wait_until(selector, |page| {
            match page.select(root, selector).as_slice() {
                [one] => Some(*one),
                _ => None,
            }
        })
        .await
    }

    /// Wait until `selector` matches nothing.
    pub async fn wait_gone(&mut self, selector: &str) {
        let root = self.dom.root();
        self.wait_until(&format!("{selector} to disappear"), |page| {
            page.select(root, selector).is_empty().then_some(())
        })
        .await
    }

    /// Keep running the UI until it has had nothing to do for 300 ms, then
    /// return. Work arriving restarts the quiet period; a page that never
    /// goes quiet within the wait deadline fails the test.
    ///
    /// Only for what a condition cannot express: observing that something
    /// did NOT happen after an action. A quiet moment is evidence of nothing
    /// arriving within 300 ms, not of nothing arriving at all, so readiness
    /// belongs to [`Page::wait_until`] on a state the page renders.
    pub async fn settle(&mut self) {
        const QUIET: Duration = Duration::from_millis(300);
        // The whole search for a quiet period is bounded, not just the
        // stretches between bursts of work: a page that is busy until just
        // before the deadline and quiet after it did not settle in time.
        let deadline = tokio::time::Instant::now() + self.wait;
        let quiet = tokio::time::timeout_at(deadline, async {
            loop {
                self.render();
                let work = self
                    .vdom
                    .wait_for_work()
                    .with_subscriber(self.quiet.clone());
                if tokio::time::timeout(QUIET, work).await.is_err() {
                    break;
                }
            }
        })
        .await
        .is_ok();
        self.render();
        if !quiet {
            panic!("{}", self.failure("the page never went quiet for 300 ms"));
        }
    }

    /// The failure text every query and wait uses: what was wanted, the
    /// page as it stands, and what the page asked the fake helm for.
    pub fn failure(&self, what: &str) -> String {
        let requests = (self.requests)();
        let log: Vec<String> = requests
            .iter()
            .map(|r| {
                let query = r
                    .query
                    .as_deref()
                    .map(|q| format!("?{q}"))
                    .unwrap_or_default();
                format!(
                    "  {} {}{query}{}",
                    r.method,
                    r.path,
                    if r.handled { "" } else { "  (UNHANDLED: 404)" }
                )
            })
            .collect();
        let root = self
            .dump_root
            .filter(|&node| self.dom.is_attached(node))
            .unwrap_or(self.dom.root());
        format!(
            "{what}\n--- page ---\n{}\n--- requests to {} ---\n{}",
            self.dom.outline(root, 40_000),
            self.base,
            log.join("\n")
        )
    }

    // --- queries -------------------------------------------------------------

    /// Attached nodes under `scope` matching a small CSS subset: descendant
    /// combinators over compounds of `tag`, `.class`, `#id`, `[attr]` and
    /// `[attr="value"]`. Enough for the class-and-attribute selectors the
    /// browser specs use; anything richer is a sign the test wants a role.
    pub fn select(&self, scope: NodeId, selector: &str) -> Vec<NodeId> {
        let parts: Vec<Compound> = split_compounds(selector)
            .iter()
            .map(|part| Compound::parse(part))
            .collect();
        let (last, ancestors) = parts.split_last().expect("empty selector");
        self.dom
            .descendants(scope)
            .into_iter()
            .filter(|&node| last.matches(&self.dom, node))
            .filter(|&node| {
                // Each earlier compound must match some ancestor, nearest
                // first. As with `querySelectorAll` on an element, those
                // ancestors may lie outside the scope; only the match itself
                // must be inside it.
                let mut at = self.dom.parent(node);
                ancestors.iter().rev().all(|compound| {
                    loop {
                        let Some(parent) = at else { break false };
                        at = self.dom.parent(parent);
                        if compound.matches(&self.dom, parent) {
                            break true;
                        }
                    }
                })
            })
            .collect()
    }

    /// Exactly one match for `selector` under `scope`, or a failure that
    /// shows the page. Use it where the browser spec's locator was strict:
    /// a second match is a test-breaking ambiguity there too.
    pub fn one_in(&self, scope: NodeId, selector: &str) -> NodeId {
        match self.select(scope, selector).as_slice() {
            [one] => *one,
            found => panic!(
                "{}",
                self.failure(&format!("expected one {selector:?}, found {}", found.len()))
            ),
        }
    }

    /// Exactly one element under `scope` with this role and accessible name.
    pub fn role_in(&self, scope: NodeId, role: &str, name: &str) -> NodeId {
        let found: Vec<NodeId> = self
            .dom
            .descendants(scope)
            .into_iter()
            .filter(|&node| {
                !self.hidden(node)
                    && self.role(node).as_deref() == Some(role)
                    && self.name(node) == name
            })
            .collect();
        match found.as_slice() {
            [one] => *one,
            _ => panic!(
                "{}",
                self.failure(&format!(
                    "expected one {role} named {name:?}, found {}",
                    found.len()
                ))
            ),
        }
    }

    /// Every visible element under `scope` with this role whose accessible
    /// name contains `needle`, ignoring case. This is Playwright's default
    /// (non-`exact`) name match, so a port can assert uniqueness the way the
    /// browser spec's strict locator did: two controls that both answer to
    /// the same loose name are an ambiguity a screen reader user meets too.
    pub fn roles_like(&self, scope: NodeId, role: &str, needle: &str) -> Vec<NodeId> {
        let needle = needle.to_lowercase();
        self.dom
            .descendants(scope)
            .into_iter()
            .filter(|&node| {
                !self.hidden(node)
                    && self.role(node).as_deref() == Some(role)
                    && self.name(node).to_lowercase().contains(&needle)
            })
            .collect()
    }

    /// The element's ARIA role: its `role` attribute, else the implicit role
    /// of the few elements the specs query by role. Not the full ARIA table.
    pub fn role(&self, node: NodeId) -> Option<String> {
        if let Some(role) = self.dom.attr(node, "role") {
            return Some(role.to_owned());
        }
        let role = match self.dom.tag(node)? {
            "button" => "button",
            "a" if self.dom.attr(node, "href").is_some() => "link",
            "input" => match self.dom.attr(node, "type").unwrap_or("text") {
                "checkbox" => "checkbox",
                "radio" => "radio",
                "button" | "submit" | "reset" => "button",
                _ => "textbox",
            },
            "textarea" => "textbox",
            "select" => "combobox",
            "dialog" => "dialog",
            // A section is a landmark only when it has a name.
            "section"
                if self.dom.attr(node, "aria-label").is_some()
                    || self.dom.attr(node, "aria-labelledby").is_some() =>
            {
                "region"
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => "heading",
            "option" => "option",
            _ => return None,
        };
        Some(role.to_owned())
    }

    /// The accessible name, simplified: `aria-label`, else the text of the
    /// `aria-labelledby` targets, else the element's own text with
    /// whitespace collapsed.
    pub fn name(&self, node: NodeId) -> String {
        // An empty label names nothing (the accessible-name algorithm skips
        // it), and the launcher's search options render `aria-label=""`.
        if let Some(label) = self
            .dom
            .attr(node, "aria-label")
            .filter(|label| !label.trim().is_empty())
        {
            return collapse(label);
        }
        if let Some(ids) = self
            .dom
            .attr(node, "aria-labelledby")
            .filter(|ids| !ids.trim().is_empty())
        {
            let text: Vec<String> = ids
                .split_whitespace()
                .filter_map(|id| {
                    self.select(self.dom.root(), &format!("#{id}"))
                        .first()
                        .copied()
                })
                .map(|target| self.dom.text(target))
                .collect();
            return collapse(&text.join(" "));
        }
        collapse(&self.dom.text(node))
    }

    /// Whether the element or an ancestor carries `hidden` or `inert`, which
    /// is the only hiding this DOM can know about without CSS.
    pub fn hidden(&self, node: NodeId) -> bool {
        let mut at = Some(node);
        while let Some(node) = at {
            if self.dom.attr(node, "hidden").is_some() || self.dom.attr(node, "inert").is_some() {
                return true;
            }
            at = self.dom.parent(node);
        }
        false
    }

    /// The node's text content with whitespace collapsed, which is how
    /// Playwright's `toHaveText` compares text.
    pub fn text(&self, node: NodeId) -> String {
        collapse(&self.dom.text(node))
    }

    // --- events --------------------------------------------------------------

    /// Click `node`: dispatch `click` at it (bubbling as in a page), then, if
    /// nothing prevented the default and it is a submit button inside a form,
    /// submit the form, which is what a browser does next.
    ///
    /// A `disabled` control refuses the click (the test fails), as a browser
    /// would never deliver it. `aria-disabled` does not: the launcher keeps
    /// Launch clickable that way so an incomplete setup can explain itself.
    /// There is no hit-testing either, so a click here cannot be intercepted
    /// by a control drawn on top; that check belongs to the browser layer.
    pub fn click(&mut self, node: NodeId) {
        assert!(
            self.dom.is_attached(node),
            "{}",
            self.failure("click on a detached node")
        );
        assert!(
            self.dom.attr(node, "disabled").is_none(),
            "{}",
            self.failure(&format!(
                "click on a disabled control: {}",
                self.describe(node)
            ))
        );
        let event = self.dispatch(
            node,
            "click",
            Rc::new(PlatformEventData::new(Box::new(
                SerializedMouseData::default(),
            ))),
        );
        if event.default_action_enabled()
            && self.is_submit(node)
            && let Some(form) = self.ancestor(node, |dom, n| dom.tag(n) == Some("form"))
        {
            let data = SerializedFormData::new(String::new(), Vec::new());
            self.dispatch(
                form,
                "submit",
                Rc::new(PlatformEventData::new(Box::new(data))),
            );
        }
        self.render();
    }

    /// Type `value` into a text control: one `input` event carrying the
    /// whole new value, which is what Dioxus handlers read.
    pub fn input(&mut self, node: NodeId, value: &str) {
        let data = SerializedFormData::new(value.to_owned(), Vec::new());
        self.dispatch(
            node,
            "input",
            Rc::new(PlatformEventData::new(Box::new(data))),
        );
        self.render();
    }

    /// Press and release `key` (a named key such as `"Enter"` or
    /// `"Escape"`, or a single character) on `node`: `keydown` then `keyup`,
    /// with no modifiers.
    ///
    /// One browser default action is synthesized because the launcher
    /// depends on it: Enter in a text field whose keydown nobody prevented
    /// performs the form's implicit submission, which clicks the form's
    /// first submit button. Other defaults (Enter or Space activating a
    /// focused button) are not; there is no focus to say which one.
    pub fn key(&mut self, node: NodeId, key: &str) {
        use dioxus::html::SerializedKeyboardData;
        use dioxus::prelude::{Code, Key, Location, Modifiers};
        let parsed: Key = key
            .parse()
            .unwrap_or_else(|_| Key::Character(key.to_owned()));
        let code = match key {
            "Enter" => Code::Enter,
            "Escape" => Code::Escape,
            "Tab" => Code::Tab,
            "ArrowDown" => Code::ArrowDown,
            "ArrowUp" => Code::ArrowUp,
            _ => Code::Unidentified,
        };
        let keyboard = || {
            let data = SerializedKeyboardData::new(
                parsed.clone(),
                code,
                Location::Standard,
                false,
                Modifiers::empty(),
                false,
            );
            Rc::new(PlatformEventData::new(Box::new(data)))
        };
        let down = self.dispatch(node, "keydown", keyboard());
        self.render();
        if key == "Enter" && down.default_action_enabled() && self.is_text_field(node) {
            let form = self.ancestor(node, |dom, n| dom.tag(n) == Some("form"));
            let submit = form.and_then(|form| {
                self.dom
                    .descendants(form)
                    .into_iter()
                    .find(|&n| self.is_submit(n))
            });
            if let Some(button) = submit.filter(|&b| self.dom.attr(b, "disabled").is_none()) {
                self.click(button);
            }
        }
        self.dispatch(node, "keyup", keyboard());
        self.render();
    }

    /// Inputs whose Enter key submits their form (not buttons, checkboxes
    /// or textareas).
    fn is_text_field(&self, node: NodeId) -> bool {
        self.dom.tag(node) == Some("input")
            && matches!(
                self.dom.attr(node, "type").unwrap_or("text"),
                "text" | "search" | "url" | "email" | "password" | "tel" | "number"
            )
    }

    /// Deliver `focus` (then `focusin`) to `node`, for handlers that react
    /// to focus, such as the launcher's folder options opening. This moves
    /// no real focus: there is none to move, and no `blur` is sent to
    /// whatever was focused before. Pair with [`Page::blur`] when a test
    /// depends on the previous control letting go.
    pub fn focus(&mut self, node: NodeId) {
        use dioxus::html::SerializedFocusData;
        for name in ["focus", "focusin"] {
            let data = Rc::new(PlatformEventData::new(Box::new(
                SerializedFocusData::default(),
            )));
            self.dispatch(node, name, data);
            self.render();
        }
    }

    /// Deliver `blur` (then `focusout`) to `node`.
    pub fn blur(&mut self, node: NodeId) {
        use dioxus::html::SerializedFocusData;
        for name in ["blur", "focusout"] {
            let data = Rc::new(PlatformEventData::new(Box::new(
                SerializedFocusData::default(),
            )));
            self.dispatch(node, name, data);
            self.render();
        }
    }

    fn is_submit(&self, node: NodeId) -> bool {
        match self.dom.tag(node) {
            Some("button") => matches!(self.dom.attr(node, "type"), None | Some("submit")),
            Some("input") => self.dom.attr(node, "type") == Some("submit"),
            _ => false,
        }
    }

    fn ancestor(&self, node: NodeId, test: impl Fn(&Dom, NodeId) -> bool) -> Option<NodeId> {
        let mut at = self.dom.parent(node);
        while let Some(n) = at {
            if test(&self.dom, n) {
                return Some(n);
            }
            at = self.dom.parent(n);
        }
        None
    }

    /// Dispatch at the nearest node Dioxus knows by id. Elements without an
    /// id are static template parts with no listeners of their own, so the
    /// event would bubble from the nearest identified ancestor anyway.
    fn dispatch(
        &mut self,
        node: NodeId,
        name: &str,
        data: Rc<PlatformEventData>,
    ) -> Event<dyn std::any::Any> {
        let mut at = Some(node);
        let id: ElementId = loop {
            match at {
                Some(n) => match self.dom.element_id(n) {
                    Some(id) if n != self.dom.root() => break id,
                    _ => at = self.dom.parent(n),
                },
                None => panic!(
                    "{}",
                    self.failure(&format!("no Dioxus element to dispatch {name} at"))
                ),
            }
        };
        let event = Event::new(data as Rc<dyn std::any::Any>, true);
        let runtime = self.vdom.runtime();
        tracing::dispatcher::with_default(&self.quiet, || {
            runtime.handle_event(name, event.clone(), id)
        });
        event
    }

    /// One-line description of a node for messages.
    pub fn describe(&self, node: NodeId) -> String {
        match self.dom.kind(node) {
            Kind::Element(tag) => {
                let class = self.dom.attr(node, "class").unwrap_or("");
                format!("<{tag} class={class:?}> {:?}", self.name(node))
            }
            other => format!("{other:?}"),
        }
    }
}

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Split a selector at its descendant combinators: whitespace outside
/// `[...]`, so a quoted attribute value may contain spaces.
fn split_compounds(selector: &str) -> Vec<String> {
    let mut parts = vec![String::new()];
    let mut depth = 0;
    for c in selector.chars() {
        match c {
            '[' => depth += 1,
            ']' => depth -= 1,
            _ => {}
        }
        if c.is_whitespace() && depth == 0 {
            if !parts.last().unwrap().is_empty() {
                parts.push(String::new());
            }
        } else {
            parts.last_mut().unwrap().push(c);
        }
    }
    parts.retain(|part| !part.is_empty());
    parts
}

/// One compound selector: optional tag, classes, id and attribute tests.
struct Compound {
    tag: Option<String>,
    classes: Vec<String>,
    id: Option<String>,
    attrs: Vec<(String, Option<String>)>,
}

impl Compound {
    fn parse(text: &str) -> Self {
        let mut compound = Compound {
            tag: None,
            classes: Vec::new(),
            id: None,
            attrs: Vec::new(),
        };
        let mut rest = text;
        let take = |rest: &mut &str| {
            let end = rest.find(['.', '#', '[']).unwrap_or(rest.len());
            let (word, tail) = rest.split_at(end);
            *rest = tail;
            word.to_owned()
        };
        let tag = take(&mut rest);
        if !tag.is_empty() {
            compound.tag = Some(tag);
        }
        while let Some(first) = rest.chars().next() {
            rest = &rest[1..];
            match first {
                '.' => compound.classes.push(take(&mut rest)),
                '#' => compound.id = Some(take(&mut rest)),
                '[' => {
                    let end = rest
                        .find(']')
                        .unwrap_or_else(|| panic!("unclosed [ in selector {text:?}"));
                    let inner = &rest[..end];
                    rest = &rest[end + 1..];
                    match inner.split_once('=') {
                        Some((name, value)) => compound.attrs.push((
                            name.to_owned(),
                            Some(value.trim_matches(['"', '\'']).to_owned()),
                        )),
                        None => compound.attrs.push((inner.to_owned(), None)),
                    }
                }
                _ => panic!("unsupported selector syntax in {text:?}"),
            }
        }
        compound
    }

    fn matches(&self, dom: &Dom, node: NodeId) -> bool {
        let Some(tag) = dom.tag(node) else {
            return false;
        };
        if self.tag.as_deref().is_some_and(|want| want != tag) {
            return false;
        }
        let classes: Vec<&str> = dom
            .attr(node, "class")
            .unwrap_or("")
            .split_whitespace()
            .collect();
        self.classes.iter().all(|c| classes.contains(&c.as_str()))
            && self
                .id
                .as_deref()
                .is_none_or(|id| dom.attr(node, "id") == Some(id))
            && self.attrs.iter().all(|(name, value)| match value {
                Some(value) => dom.attr(node, name) == Some(value.as_str()),
                None => dom.attr(node, name).is_some(),
            })
    }
}
