use html2text::{Handle, RcDom};
use html5ever::interface::TreeSink;

use super::dom::{self, name};

/// Drops a linked image whose alt the next text repeats, as a product or
/// course card captions its picture with the same title.
pub fn drop_repeated_images(dom: &RcDom) {
    let mut pending = None;
    visit(dom, &dom.document, &mut pending);
}

/// An image link and its alt, lowercased, waiting for the text after it.
type Pending = Option<(Handle, String)>;

fn visit(dom: &RcDom, node: &Handle, pending: &mut Pending) {
    for child in dom::children(node) {
        if let Some(alt) = image_link_alt(&child) {
            *pending = Some((child, alt));
            continue;
        }

        let is_element = name(&child).is_some();
        if let Some((link, alt)) = pending.take() {
            let text = dom::text(&child).to_lowercase();
            if text == alt {
                dom.remove_from_parent(&link);
                continue;
            }

            // Nothing shown yet, or the repeat sits deeper, as a link inside
            // the caption cell: keep waiting.
            if text.is_empty() || (is_element && text.starts_with(&alt)) {
                *pending = Some((link, alt));
            }
        }

        if is_element {
            visit(dom, &child, pending);
        }
    }
}

/// The alts of a link that shows only images, single-spaced and lowercased.
fn image_link_alt(node: &Handle) -> Option<String> {
    if name(node) != Some("a") || !dom::text(node).is_empty() {
        return None;
    }

    let alts = dom::alts(node).join(" ");
    let alt = alts.split_whitespace().collect::<Vec<_>>().join(" ");
    (!alt.is_empty()).then(|| alt.to_lowercase())
}
