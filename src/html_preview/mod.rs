//! An HTML part as Preview lines: html2text lays it out, after a pass over
//! its DOM that drops what mail hides and reshapes layout tables.

mod clean;
mod decorator;
mod dom;
mod hidden;
mod lines;
mod tables;

use html2text::config::{self, ImageRenderMode};

use crate::styled_line::StyledLine;

/// `html` laid out in lines at most `width` columns wide, where it can be.
pub fn render(html: &str, width: usize) -> Result<Vec<StyledLine>, html2text::Error> {
    let config = config::with_decorator(decorator::Decorator)
        .use_doc_css()
        .no_table_borders()
        .allow_width_overflow()
        .link_footnotes(false)
        .empty_img_mode(ImageRenderMode::IgnoreEmpty);
    let dom = config.parse_html(html.as_bytes())?;
    clean::clean(&dom, width);
    let tree = config.dom_to_render_tree(&dom)?;
    Ok(lines::styled(config.render_to_lines(tree, width)?))
}
