use html2text::{Element, Handle, RcDom};
use html5ever::{
    LocalName, QualName,
    interface::{NodeOrText, TreeSink},
    ns,
    tendril::StrTendril,
};
use unicode_width::UnicodeWidthStr;

use crate::html_text;

pub fn name(node: &Handle) -> Option<&str> {
    match &node.data {
        Element { name, .. } => Some(&name.local),
        _ => None,
    }
}

pub fn attr(node: &Handle, key: &str) -> Option<String> {
    match &node.data {
        Element { attrs, .. } => attrs
            .borrow()
            .iter()
            .find(|a| &*a.name.local == key)
            .map(|a| a.value.to_string()),
        _ => None,
    }
}

pub fn children(node: &Handle) -> Vec<Handle> {
    node.children.borrow().clone()
}

/// Whether an element below `node` satisfies `test`.
pub fn any_below(node: &Handle, test: &impl Fn(&Handle) -> bool) -> bool {
    node.children
        .borrow()
        .iter()
        .any(|child| test(child) || any_below(child, test))
}

pub fn element(dom: &RcDom, name: &str) -> Handle {
    let name = QualName::new(None, ns!(html), LocalName::from(name));
    dom.create_element(
        name,
        Vec::new(),
        html5ever::interface::ElementFlags::default(),
    )
}

pub fn append_text(dom: &RcDom, parent: &Handle, text: &str) {
    dom.append(parent, NodeOrText::AppendText(StrTendril::from(text)));
}

pub fn append(dom: &RcDom, parent: &Handle, child: Handle) {
    dom.append(parent, NodeOrText::AppendNode(child));
}

/// Moves every child of `from` to the end of `to`.
pub fn adopt_children(dom: &RcDom, from: &Handle, to: &Handle) {
    for child in children(from) {
        dom.remove_from_parent(&child);
        append(dom, to, child);
    }
}

pub fn replace(dom: &RcDom, old: &Handle, new: Handle) {
    dom.append_before_sibling(old, NodeOrText::AppendNode(new));
    dom.remove_from_parent(old);
}

/// The words under `node`, single-spaced. Serialised and stripped, since
/// html2text keeps the text node variant out of reach.
pub fn text(node: &Handle) -> String {
    let mut html = Vec::new();
    // Writing to a Vec cannot fail.
    node.serialize(&mut html).ok();
    let text = html_text::strip(&String::from_utf8_lossy(&html));
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The columns `node` takes on one line: its words, and `[alt]` for each
/// image left inside it.
pub fn width(node: &Handle) -> usize {
    let images: usize = alts(node).iter().map(|alt| alt.trim().width() + 3).sum();
    text(node).width() + images
}

/// The `alt` of every image below `node` that has one, in document order.
pub fn alts(node: &Handle) -> Vec<String> {
    let mut alts = Vec::new();
    let mut stack = vec![node.clone()];
    while let Some(next) = stack.pop() {
        if name(&next) == Some("img")
            && let Some(alt) = attr(&next, "alt")
        {
            alts.push(alt);
        }

        stack.extend(children(&next).into_iter().rev());
    }

    alts
}

pub fn has_image(node: &Handle) -> bool {
    any_below(node, &|n| matches!(name(n), Some("img" | "svg")))
}
