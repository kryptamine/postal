use html2text::{Handle, RcDom};
use html5ever::interface::TreeSink;

use super::{
    dom::{self, name},
    hidden, tables,
};

/// Readies the DOM of a Message for html2text at `width`: drops what a mail
/// client hides and every image outside a link, labels a link left empty by
/// a dropped image `[link]`, and turns tables into blocks unless they read as data.
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
        let had_image = link && dom::has_image(&child);
        let layout = is_table && tables::layout(&child);
        clean_below(dom, &child, in_link || link, width);
        if link && dom::text(&child).is_empty() && !dom::has_image(&child) {
            // A browser shows an empty link as nothing; one whose only image
            // was dropped is still a visible link there.
            if had_image && dom::attr(&child, "href").is_some() {
                dom::append_text(dom, &child, "[link]");
            } else {
                dom.remove_from_parent(&child);
                continue;
            }
        }

        if layout {
            tables::flatten(dom, &child);
        } else if is_table && !tables::fits(&child, width) {
            tables::stack(dom, &child);
        }
    }
}
