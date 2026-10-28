//! An in-memory DOM that a headless [`VirtualDom`](dioxus::prelude::VirtualDom)
//! writes into, so tests can read the page the way a browser test does.
//!
//! The renderer contract is dioxus-core's [`WriteMutations`]: a small stack
//! machine that loads template clones, assigns element ids along template
//! paths, fills dynamic placeholders, and edits nodes by id. The browser's
//! interpreter (`dioxus-interpreter-js`) and this module implement the same
//! machine, so a test sees the same tree, attributes and text a page would,
//! across nested components and later re-renders. What it deliberately does
//! not model is everything a browser adds on top: layout, CSS, focus, and
//! page JavaScript. Tests that depend on those stay in the browser suite.
//!
//! Nodes live in an arena and are never freed; a removed subtree is only
//! detached. Tests are short-lived, and keeping detached nodes makes a stale
//! [`NodeId`] read as "detached" instead of aliasing a newer node.

use dioxus::core::{
    AttributeValue, ElementId, Template, TemplateAttribute, TemplateNode, WriteMutations,
};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Index of a node in the [`Dom`] arena. Stable for the life of the DOM.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(usize);

/// What a node is. Placeholders are Dioxus's anchors for empty dynamic
/// slots (a `None` child, an empty list); they render nothing.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Element(&'static str),
    Text(String),
    Placeholder,
}

#[derive(Clone, Debug)]
struct Node {
    kind: Kind,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    /// Attribute name to value. Namespaced attributes (Dioxus's `style`
    /// namespace carries individual CSS properties) are keyed `ns:name`.
    attrs: BTreeMap<String, String>,
    listeners: HashSet<&'static str>,
    element_id: Option<ElementId>,
}

/// Attributes the browser treats as present-or-absent. Dioxus sends
/// `disabled: false` as the text "false"; the browser interpreter removes the
/// attribute for these instead of storing that text, and so does this DOM, so
/// `has_attr("disabled")` means what it means in a page.
const BOOLEAN_ATTRS: &[&str] = &[
    "allowfullscreen",
    "async",
    "autofocus",
    "autoplay",
    "checked",
    "controls",
    "default",
    "defer",
    "disabled",
    "formnovalidate",
    "hidden",
    "inert",
    "ismap",
    "itemscope",
    "loop",
    "multiple",
    "muted",
    "nomodule",
    "novalidate",
    "open",
    "playsinline",
    "readonly",
    "required",
    "reversed",
    "selected",
    "truespeed",
];

/// Where [`Dom::place`] puts nodes.
enum Place {
    End(NodeId),
    Before(NodeId),
    After(NodeId),
    /// Where this node was; the node itself is detached.
    Instead(NodeId),
}

/// The tree a [`VirtualDom`](dioxus::prelude::VirtualDom) renders into.
///
/// Node 0 is the mount root and carries [`ElementId(0)`](ElementId), which is
/// where Dioxus appends the app's top-level nodes.
pub struct Dom {
    nodes: Vec<Node>,
    by_element: HashMap<ElementId, NodeId>,
    stack: Vec<NodeId>,
}

impl Default for Dom {
    fn default() -> Self {
        let mut dom = Dom {
            nodes: Vec::new(),
            by_element: HashMap::new(),
            stack: Vec::new(),
        };
        let root = dom.alloc(Kind::Element("body"));
        dom.bind(root, ElementId(0));
        dom
    }
}

impl Dom {
    /// The mount root (`<body>` in effect).
    pub fn root(&self) -> NodeId {
        NodeId(0)
    }

    pub fn kind(&self, node: NodeId) -> &Kind {
        &self.nodes[node.0].kind
    }

    pub fn tag(&self, node: NodeId) -> Option<&'static str> {
        match self.nodes[node.0].kind {
            Kind::Element(tag) => Some(tag),
            _ => None,
        }
    }

    pub fn parent(&self, node: NodeId) -> Option<NodeId> {
        self.nodes[node.0].parent
    }

    pub fn children(&self, node: NodeId) -> &[NodeId] {
        &self.nodes[node.0].children
    }

    pub fn attr(&self, node: NodeId, name: &str) -> Option<&str> {
        self.nodes[node.0].attrs.get(name).map(String::as_str)
    }

    pub fn attrs(&self, node: NodeId) -> impl Iterator<Item = (&str, &str)> {
        self.nodes[node.0]
            .attrs
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn element_id(&self, node: NodeId) -> Option<ElementId> {
        self.nodes[node.0].element_id
    }

    /// Whether the node is still reachable from the mount root.
    pub fn is_attached(&self, node: NodeId) -> bool {
        let mut at = node;
        loop {
            if at == self.root() {
                return true;
            }
            match self.parent(at) {
                Some(parent) => at = parent,
                None => return false,
            }
        }
    }

    /// Every attached node below `node`, in document order, `node` excluded.
    pub fn descendants(&self, node: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut pending: Vec<NodeId> = self.children(node).iter().rev().copied().collect();
        while let Some(next) = pending.pop() {
            out.push(next);
            pending.extend(self.children(next).iter().rev().copied());
        }
        out
    }

    /// `textContent`: every descendant text node, concatenated.
    pub fn text(&self, node: NodeId) -> String {
        let mut out = String::new();
        if let Kind::Text(text) = self.kind(node) {
            out.push_str(text);
        }
        for child in self.descendants(node) {
            if let Kind::Text(text) = self.kind(child) {
                out.push_str(text);
            }
        }
        out
    }

    /// An indented rendering of the subtree, for failure messages. Bounded so
    /// a whole-app dump cannot flood the test output.
    pub fn outline(&self, node: NodeId, limit: usize) -> String {
        let mut out = String::new();
        self.outline_into(node, 0, &mut out);
        if out.len() > limit {
            let mut cut = limit;
            while !out.is_char_boundary(cut) {
                cut -= 1;
            }
            out.truncate(cut);
            out.push_str("\n…(truncated)");
        }
        out
    }

    fn outline_into(&self, node: NodeId, depth: usize, out: &mut String) {
        let pad = "  ".repeat(depth);
        match self.kind(node) {
            Kind::Element(tag) => {
                out.push_str(&format!("{pad}<{tag}"));
                for (name, value) in self.attrs(node) {
                    // Icon markup and tooltip prose dominate a dump without
                    // telling a failing test anything; keep values short.
                    let shown: String = value.chars().take(80).collect();
                    let more = if shown.len() < value.len() { "…" } else { "" };
                    out.push_str(&format!(" {name}={shown:?}{more}"));
                }
                // Which events a node listens for is what a failing test
                // most often needs to know ("is this the clickable one?").
                let mut listeners: Vec<&str> =
                    self.nodes[node.0].listeners.iter().copied().collect();
                if !listeners.is_empty() {
                    listeners.sort_unstable();
                    out.push_str(&format!(" on=[{}]", listeners.join(",")));
                }
                out.push_str(">\n");
                if *tag != "svg" {
                    for &child in self.children(node) {
                        self.outline_into(child, depth + 1, out);
                    }
                }
            }
            Kind::Text(text) if !text.trim().is_empty() => {
                out.push_str(&format!("{pad}{text:?}\n"))
            }
            _ => {}
        }
    }

    // --- arena and stack primitives -------------------------------------

    fn alloc(&mut self, kind: Kind) -> NodeId {
        self.nodes.push(Node {
            kind,
            parent: None,
            children: Vec::new(),
            attrs: BTreeMap::new(),
            listeners: HashSet::new(),
            element_id: None,
        });
        NodeId(self.nodes.len() - 1)
    }

    /// Point an [`ElementId`] at a node. Dioxus reuses ids after removal, so
    /// a rebind simply moves the id to its new owner.
    fn bind(&mut self, node: NodeId, id: ElementId) {
        self.nodes[node.0].element_id = Some(id);
        self.by_element.insert(id, node);
    }

    fn node(&self, id: ElementId) -> NodeId {
        *self
            .by_element
            .get(&id)
            .unwrap_or_else(|| panic!("mutation names unknown element {id:?}"))
    }

    fn detach(&mut self, node: NodeId) {
        if let Some(parent) = self.nodes[node.0].parent.take() {
            self.nodes[parent.0].children.retain(|&child| child != node);
        }
    }

    fn pop(&mut self, m: usize) -> Vec<NodeId> {
        let at = self
            .stack
            .len()
            .checked_sub(m)
            .expect("mutation pops more nodes than the stack holds");
        self.stack.split_off(at)
    }

    /// Put `nodes`, in order, at `place`.
    ///
    /// Every node is detached from wherever it was first, and only then is
    /// the insertion index taken from the anchor. Keyed-list diffing moves
    /// live nodes within one parent, and an index taken before the detach
    /// would land a node moved from before its anchor one slot too late.
    fn place(&mut self, nodes: Vec<NodeId>, place: Place) {
        for &node in &nodes {
            self.detach(node);
        }
        let (parent, index) = match place {
            Place::End(parent) => (parent, self.children(parent).len()),
            Place::Before(anchor) => self.position(anchor),
            Place::After(anchor) => {
                let (parent, index) = self.position(anchor);
                (parent, index + 1)
            }
            Place::Instead(old) => {
                let at = self.position(old);
                self.detach(old);
                at
            }
        };
        for (offset, node) in nodes.into_iter().enumerate() {
            self.nodes[node.0].parent = Some(parent);
            self.nodes[parent.0].children.insert(index + offset, node);
        }
    }

    fn position(&self, node: NodeId) -> (NodeId, usize) {
        let parent = self
            .parent(node)
            .expect("mutation positions relative to a detached node");
        let index = self
            .children(parent)
            .iter()
            .position(|&child| child == node)
            .expect("parent lists its child");
        (parent, index)
    }

    /// The node at `path` below the top of the stack: child indices into the
    /// template clone, exactly as the browser interpreter walks them.
    fn at_path(&self, path: &[u8]) -> NodeId {
        let mut node = *self.stack.last().expect("path lookup with an empty stack");
        for &index in path {
            node = self.children(node)[index as usize];
        }
        node
    }

    fn clone_template(&mut self, template: &TemplateNode) -> NodeId {
        match template {
            TemplateNode::Element {
                tag,
                attrs,
                children,
                ..
            } => {
                let node = self.alloc(Kind::Element(tag));
                for attr in attrs.iter() {
                    if let TemplateAttribute::Static {
                        name,
                        value,
                        namespace,
                    } = attr
                    {
                        self.set_attr(node, name, *namespace, Some(value.to_string()));
                    }
                }
                for child in children.iter() {
                    let child = self.clone_template(child);
                    self.nodes[child.0].parent = Some(node);
                    self.nodes[node.0].children.push(child);
                }
                node
            }
            TemplateNode::Text { text } => self.alloc(Kind::Text(text.to_string())),
            TemplateNode::Dynamic { .. } => self.alloc(Kind::Placeholder),
        }
    }

    fn set_attr(
        &mut self,
        node: NodeId,
        name: &str,
        namespace: Option<&str>,
        value: Option<String>,
    ) {
        let key = match namespace {
            Some(ns) => format!("{ns}:{name}"),
            None => name.to_owned(),
        };
        let boolean = namespace.is_none() && BOOLEAN_ATTRS.contains(&name);
        let value = value.filter(|value| !(boolean && (value == "false")));
        match value {
            Some(value) => {
                self.nodes[node.0].attrs.insert(key, value);
            }
            None => {
                self.nodes[node.0].attrs.remove(&key);
            }
        }
    }
}

impl WriteMutations for Dom {
    fn append_children(&mut self, id: ElementId, m: usize) {
        let parent = self.node(id);
        let nodes = self.pop(m);
        self.place(nodes, Place::End(parent));
    }

    fn assign_node_id(&mut self, path: &'static [u8], id: ElementId) {
        let node = self.at_path(path);
        self.bind(node, id);
    }

    fn create_placeholder(&mut self, id: ElementId) {
        let node = self.alloc(Kind::Placeholder);
        self.bind(node, id);
        self.stack.push(node);
    }

    fn create_text_node(&mut self, value: &str, id: ElementId) {
        let node = self.alloc(Kind::Text(value.to_owned()));
        self.bind(node, id);
        self.stack.push(node);
    }

    fn load_template(&mut self, template: Template, index: usize, id: ElementId) {
        let node = self.clone_template(&template.roots[index]);
        self.bind(node, id);
        self.stack.push(node);
    }

    fn replace_node_with(&mut self, id: ElementId, m: usize) {
        let old = self.node(id);
        let nodes = self.pop(m);
        self.place(nodes, Place::Instead(old));
    }

    fn replace_placeholder_with_nodes(&mut self, path: &'static [u8], m: usize) {
        // Pop first: the path is relative to the template root left on top.
        let nodes = self.pop(m);
        let old = self.at_path(path);
        self.place(nodes, Place::Instead(old));
    }

    fn insert_nodes_after(&mut self, id: ElementId, m: usize) {
        let anchor = self.node(id);
        let nodes = self.pop(m);
        self.place(nodes, Place::After(anchor));
    }

    fn insert_nodes_before(&mut self, id: ElementId, m: usize) {
        let anchor = self.node(id);
        let nodes = self.pop(m);
        self.place(nodes, Place::Before(anchor));
    }

    fn set_attribute(
        &mut self,
        name: &'static str,
        ns: Option<&'static str>,
        value: &AttributeValue,
        id: ElementId,
    ) {
        let node = self.node(id);
        let value = match value {
            AttributeValue::Text(text) => Some(text.clone()),
            AttributeValue::Float(f) => Some(f.to_string()),
            AttributeValue::Int(n) => Some(n.to_string()),
            AttributeValue::Bool(b) => Some(b.to_string()),
            AttributeValue::None => None,
            // Listeners arrive through `create_event_listener`; opaque `Any`
            // values have no DOM representation in a browser either.
            AttributeValue::Listener(_) | AttributeValue::Any(_) => return,
        };
        self.set_attr(node, name, ns, value);
    }

    fn set_node_text(&mut self, value: &str, id: ElementId) {
        let node = self.node(id);
        self.nodes[node.0].kind = Kind::Text(value.to_owned());
    }

    fn create_event_listener(&mut self, name: &'static str, id: ElementId) {
        let node = self.node(id);
        self.nodes[node.0].listeners.insert(name);
    }

    fn remove_event_listener(&mut self, name: &'static str, id: ElementId) {
        let node = self.node(id);
        self.nodes[node.0].listeners.remove(name);
    }

    fn remove_node(&mut self, id: ElementId) {
        let node = self.node(id);
        self.detach(node);
    }

    fn push_root(&mut self, id: ElementId) {
        let node = self.node(id);
        self.stack.push(node);
    }
}
