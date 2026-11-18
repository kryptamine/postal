use html2text::{Handle, RcDom};
use html5ever::interface::TreeSink;

use super::{
    dom::{self, name},
    hidden, tables,
};

/// Readies the DOM of a Message for html2text at `width`: drops what a mail
/// client hides and every image outside a link, gives an empty link a
/// `[link]` label, and turns tables into blocks unless they read as data.
pub fn clean(dom: &RcDom, width: usize) {
    clean_below(dom, &dom.document, false, width);
}

fn clean_below(dom: &RcDom, node: &Handle, in_link: bool, width: usize) {
    for child in dom::children(node) {
        let Some(tag) = name(&child) else {
            continue;
        };

        let dropped = match tag {
            "script" | "template" | "noscript" => true,
            "img" => {
                !in_link
                    || hidden::pixel(&child)
                    || dom::attr(&child, "alt").is_none_or(|alt| alt.trim().is_empty())
            }
            "svg" => !in_link,
            _ => false,
        };

        if dropped || hidden::hidden(&child) {
            dom.remove_from_parent(&child);
            continue;
        }

        let is_table = tag == "table";
        let link = tag == "a";
        let layout = is_table && tables::layout(&child);
        clean_below(dom, &child, in_link || link, width);
        if link
            && dom::attr(&child, "href").is_some()
            && dom::text(&child).is_empty()
            && !dom::has_image(&child)
        {
            dom::append_text(dom, &child, "[link]");
        }

        if layout {
            tables::flatten(dom, &child);
        } else if is_table && !tables::fits(&child, width) {
            tables::stack(dom, &child);
        }
    }
}
