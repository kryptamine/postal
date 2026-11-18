use html2text::{Handle, RcDom};

use super::dom::{self, any_below, name};

/// Room kept between two cells side by side: html2text puts one column
/// there, and the second is slack.
const GAP: usize = 2;

/// Whether `table` only lays out the mail, as most tables in mail do:
/// marked so, holding a table, or a single row or column without headers.
pub fn layout(table: &Handle) -> bool {
    if dom::attr(table, "role").is_some_and(|role| role == "presentation")
        || any_below(table, &|n| name(n) == Some("table"))
    {
        return true;
    }

    if any_below(table, &|n| name(n) == Some("th")) {
        return false;
    }

    let rows = rows(table);
    rows.len() <= 1 || rows.iter().all(|row| cells(row).len() <= 1)
}

/// Replaces a layout table by its cells, one block each, in reading order.
pub fn flatten(dom: &RcDom, table: &Handle) {
    let blocks = dom::element(dom, "div");
    for cell in rows(table).iter().flat_map(cells) {
        let block = dom::element(dom, "div");
        dom::adopt_children(dom, &cell, &block);
        dom::append(dom, &blocks, block);
    }

    dom::replace(dom, table, blocks);
}

/// Whether every cell of a data table fits on one line at `width`.
pub fn fits(table: &Handle, width: usize) -> bool {
    let mut columns: Vec<usize> = Vec::new();
    for row in rows(table) {
        for (index, cell) in cells(&row).iter().enumerate() {
            let cell = dom::width(cell);
            match columns.get_mut(index) {
                Some(column) => *column = (*column).max(cell),
                None => columns.push(cell),
            }
        }
    }

    columns.iter().sum::<usize>() + GAP * columns.len().saturating_sub(1) <= width
}

/// Replaces a data table by one paragraph per row, a cell per line, each
/// led by its column's header in bold. Cells left empty, as icon-only
/// cells are once their images go, are dropped.
pub fn stack(dom: &RcDom, table: &Handle) {
    let rows = rows(table);
    let headers: Option<Vec<String>> = rows
        .first()
        .filter(|row| cells(row).iter().all(|cell| name(cell) == Some("th")))
        .map(|row| cells(row).iter().map(dom::text).collect());
    let paragraphs = dom::element(dom, "div");
    for row in rows.iter().skip(usize::from(headers.is_some())) {
        let paragraph = dom::element(dom, "p");
        for (index, cell) in cells(row).iter().enumerate() {
            if dom::text(cell).is_empty() && !dom::has_image(cell) {
                continue;
            }

            let line = dom::element(dom, "div");
            let header = headers.as_ref().and_then(|h| h.get(index));
            if let Some(header) = header.filter(|h| !h.is_empty()) {
                let bold = dom::element(dom, "b");
                dom::append_text(dom, &bold, &format!("{header}: "));
                dom::append(dom, &line, bold);
            }

            dom::adopt_children(dom, cell, &line);
            dom::append(dom, &paragraph, line);
        }

        dom::append(dom, &paragraphs, paragraph);
    }

    dom::replace(dom, table, paragraphs);
}

/// The rows of `table` itself, not of a table inside it.
fn rows(table: &Handle) -> Vec<Handle> {
    let mut rows = Vec::new();
    for child in dom::children(table) {
        match name(&child) {
            Some("tr") => rows.push(child),
            Some("thead" | "tbody" | "tfoot") => rows.extend(
                dom::children(&child)
                    .into_iter()
                    .filter(|row| name(row) == Some("tr")),
            ),
            _ => {}
        }
    }

    rows
}

fn cells(row: &Handle) -> Vec<Handle> {
    dom::children(row)
        .into_iter()
        .filter(|cell| matches!(name(cell), Some("td" | "th")))
        .collect()
}
