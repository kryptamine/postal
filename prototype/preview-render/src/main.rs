// PROTOTYPE, throwaway. Renders a folder of real .eml through the candidate
// Preview converter (mail-parser + html2text + Postal DOM pass + decorator).
// Usage: cargo run --release -- <dir>   |   cargo run --release -- <dir> --dump <out>
use std::cell::RefCell;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::time::Instant;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use html2text::config::{self, ImageRenderMode};
use html2text::render::{TaggedLineElement, TextDecorator};
use html2text::{Colour, Element, Handle, RcDom};
use html5ever::interface::{NodeOrText, TreeSink};
use html5ever::tendril::StrTendril;
use html5ever::{ns, LocalName, QualName};
use mail_parser::{MessageParser, PartType};
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Paragraph},
    DefaultTerminal, Frame,
};

// ---------- corpus ----------

struct Msg {
    name: String,
    subject: String,
    from: String,
    date: String,
    html: Option<String>,
    text: Option<String>,
}

fn load(dir: &Path) -> Vec<Msg> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("read corpus dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "eml"))
        .collect();
    paths.sort();
    paths
        .iter()
        .filter_map(|p| {
            let raw = std::fs::read(p).ok()?;
            let m = MessageParser::default().parse(&raw)?;
            let html = m.html_part(0).and_then(|p| match &p.body {
                PartType::Html(s) => Some(s.to_string()),
                _ => None,
            });
            let text = m.text_part(0).and_then(|p| match &p.body {
                PartType::Text(s) => Some(s.to_string()),
                _ => None,
            });
            let from = m
                .from()
                .and_then(|a| a.first())
                .map(|a| match (a.name(), a.address()) {
                    (Some(n), Some(e)) => format!("{n} <{e}>"),
                    (_, Some(e)) => e.to_string(),
                    (Some(n), None) => n.to_string(),
                    _ => String::new(),
                })
                .unwrap_or_default();
            Some(Msg {
                name: p.file_name()?.to_string_lossy().into_owned(),
                subject: m.subject().unwrap_or("(no subject)").to_string(),
                from,
                date: m.date().map(|d| d.to_rfc822()).unwrap_or_default(),
                html,
                text,
            })
        })
        .collect()
}

// ---------- options ----------

#[derive(Clone, Copy, PartialEq, Debug)]
enum ColourMode {
    Off,
    Semantic,
    Html,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum View {
    Rendered,
    Plain,
    RawHtml,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum LinkMode {
    Inline,
    Footnotes,
    Both,
}

#[derive(Clone, Copy, PartialEq)]
struct Opts {
    links: LinkMode,
    css: bool,
    hide: bool,
    flatten: bool,
    borders: bool,
    stack: bool,
    colour: ColourMode,
    view: View,
    width: Option<usize>,
    pager_ansi: bool,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            links: LinkMode::Inline,
            css: true,
            hide: true,
            flatten: true,
            borders: false,
            stack: true,
            colour: ColourMode::Semantic,
            view: View::Rendered,
            width: None,
            pager_ansi: true,
        }
    }
}

// ---------- decorator ----------

#[derive(Clone, Debug, PartialEq, Eq, Default)]
enum Ann {
    #[default]
    None,
    Link(usize),
    Marker,
    Image,
    Em,
    Strong,
    Code,
    Pre,
    Colour(u8, u8, u8),
}

#[derive(Default)]
struct Notes {
    urls: Vec<String>,
    open: Vec<Option<usize>>,
    colours: Vec<bool>,
}

#[derive(Clone)]
struct Deco {
    st: Rc<RefCell<Notes>>,
    html_colours: bool,
    markers: bool,
}

impl TextDecorator for Deco {
    type Annotation = Ann;

    fn decorate_link_start(&mut self, url: &str) -> (String, Ann) {
        let mut st = self.st.borrow_mut();
        let n = if url.starts_with('#') || url.is_empty() {
            None
        } else {
            let url = url.strip_prefix("mailto:").unwrap_or(url);
            Some(match st.urls.iter().position(|u| u == url) {
                Some(i) => i + 1,
                None => {
                    st.urls.push(url.to_string());
                    st.urls.len()
                }
            })
        };
        st.open.push(n);
        (String::new(), Ann::Link(n.unwrap_or(0)))
    }
    fn decorate_link_end(&mut self) -> String {
        match self.st.borrow_mut().open.pop().flatten() {
            Some(n) if self.markers => format!("\u{e000}[{n}]"),
            _ => String::new(),
        }
    }
    fn decorate_em_start(&self) -> (String, Ann) {
        (String::new(), Ann::Em)
    }
    fn decorate_em_end(&self) -> String {
        String::new()
    }
    fn decorate_strong_start(&self) -> (String, Ann) {
        (String::new(), Ann::Strong)
    }
    fn decorate_strong_end(&self) -> String {
        String::new()
    }
    fn decorate_strikeout_start(&self) -> (String, Ann) {
        (String::new(), Ann::None)
    }
    fn decorate_strikeout_end(&self) -> String {
        String::new()
    }
    fn decorate_code_start(&self) -> (String, Ann) {
        (String::new(), Ann::Code)
    }
    fn decorate_code_end(&self) -> String {
        String::new()
    }
    fn decorate_preformat_first(&self) -> Ann {
        Ann::Pre
    }
    fn decorate_preformat_cont(&self) -> Ann {
        Ann::Pre
    }
    fn decorate_image(&mut self, src: &str, title: &str) -> (String, Ann) {
        let _ = src;
        if self.st.borrow().open.is_empty() || title.trim().is_empty() {
            return (String::new(), Ann::None);
        }
        (format!("[{}]", title.trim()), Ann::Image)
    }
    fn header_prefix(&self, _level: usize) -> String {
        String::new()
    }
    fn quote_prefix(&self) -> String {
        "> ".into()
    }
    fn unordered_item_prefix(&self) -> String {
        "• ".into()
    }
    fn ordered_item_prefix(&self, i: i64) -> String {
        format!("{i}. ")
    }
    fn make_subblock_decorator(&self) -> Self {
        self.clone()
    }
    fn push_colour(&mut self, c: Colour) -> Option<Ann> {
        self.st.borrow_mut().colours.push(self.html_colours);
        self.html_colours.then_some(Ann::Colour(c.r, c.g, c.b))
    }
    fn pop_colour(&mut self) -> bool {
        self.st.borrow_mut().colours.pop().unwrap_or(false)
    }
    fn finalise(&mut self, _urls: Vec<String>) -> Vec<html2text::render::TaggedLine<Ann>> {
        Vec::new()
    }
}

// ---------- DOM pass ----------

#[derive(Default, Clone, Copy)]
struct PassStats {
    hidden: usize,
    pixels: usize,
    flattened: usize,
    data_tables: usize,
    stacked: usize,
}

fn tag(h: &Handle) -> Option<&str> {
    match &h.data {
        Element { name, .. } => Some(&name.local),
        _ => None,
    }
}

fn attr(h: &Handle, key: &str) -> Option<String> {
    match &h.data {
        Element { attrs, .. } => attrs
            .borrow()
            .iter()
            .find(|a| &*a.name.local == key)
            .map(|a| a.value.to_string()),
        _ => None,
    }
}

fn zero_val(s: &str, prop: &str) -> bool {
    let mut rest = s;
    while let Some(i) = rest.find(prop) {
        let v = &rest[i + prop.len()..];
        let v = v.trim_start_matches('0');
        if v.len() < rest[i + prop.len()..].len()
            && (v.is_empty() || v.starts_with(';') || v.starts_with("px") || v.starts_with('!'))
        {
            return true;
        }
        rest = &rest[i + prop.len()..];
    }
    false
}

fn is_hidden(h: &Handle) -> bool {
    if attr(h, "hidden").is_some() || attr(h, "aria-hidden").is_some_and(|v| v == "true") && tag(h) != Some("img") && false {
        return true;
    }
    let Some(style) = attr(h, "style") else { return false };
    let s: String = style.to_ascii_lowercase().chars().filter(|c| !c.is_whitespace()).collect();
    s.contains("display:none")
        || s.contains("visibility:hidden")
        || s.contains("mso-hide:all")
        || zero_val(&s, "opacity:")
        || (zero_val(&s, "font-size:") && !resets_font(h))
        || (zero_val(&s, "max-height:") && s.contains("overflow:hidden"))
}

// MJML wrappers set font-size:0 to kill inline-block gaps, then reset it inside.
fn resets_font(h: &Handle) -> bool {
    h.children.borrow().iter().any(|c| {
        attr(c, "style").is_some_and(|st| {
            let st = st.to_ascii_lowercase().replace(' ', "");
            st.contains("font-size:") && !zero_val(&st, "font-size:")
        }) || resets_font(c)
    })
}

fn is_pixel(h: &Handle) -> bool {
    let tiny = |v: Option<String>| v.is_some_and(|v| matches!(v.trim(), "0" | "1" | "0px" | "1px"));
    tiny(attr(h, "width")) || tiny(attr(h, "height")) || {
        let s = attr(h, "style").unwrap_or_default().to_ascii_lowercase().replace(' ', "");
        s.contains("width:1px") || s.contains("height:1px") || s.contains("width:0") || s.contains("height:0")
    }
}

fn rows(table: &Handle) -> Vec<Handle> {
    let mut out = Vec::new();
    for c in table.children.borrow().iter() {
        match tag(c) {
            Some("tr") => out.push(c.clone()),
            Some("tbody" | "thead" | "tfoot") => {
                out.extend(c.children.borrow().iter().filter(|r| tag(r) == Some("tr")).cloned());
            }
            _ => {}
        }
    }
    out
}

fn cells(row: &Handle) -> Vec<Handle> {
    row.children.borrow().iter().filter(|c| matches!(tag(c), Some("td" | "th"))).cloned().collect()
}

fn has_alt_img(h: &Handle) -> bool {
    h.children.borrow().iter().any(|c| {
        (tag(c) == Some("img") && attr(c, "alt").is_some_and(|a| !a.trim().is_empty())) || has_alt_img(c)
    })
}

fn has_desc(h: &Handle, name: &str) -> bool {
    h.children.borrow().iter().any(|c| tag(c) == Some(name) || has_desc(c, name))
}

fn is_layout(table: &Handle) -> bool {
    if attr(table, "role").is_some_and(|r| r == "presentation") || has_desc(table, "table") {
        return true;
    }
    if has_desc(table, "th") {
        return false;
    }
    let rs = rows(table);
    rs.len() <= 1 || rs.iter().all(|r| cells(r).len() <= 1)
}

fn div(dom: &RcDom) -> Handle {
    dom.create_element(QualName::new(None, ns!(html), LocalName::from("div")), vec![], Default::default())
}

fn clean(dom: &RcDom, node: &Handle, o: Opts, width: usize, st: &mut PassStats) {
    let kids: Vec<Handle> = node.children.borrow().clone();
    for c in kids {
        let Some(name) = tag(&c).map(str::to_owned) else { continue };
        if matches!(name.as_str(), "script" | "template" | "noscript") {
            dom.remove_from_parent(&c);
            continue;
        }
        if o.hide && name != "style" && name != "head" && is_hidden(&c) {
            dom.remove_from_parent(&c);
            st.hidden += 1;
            continue;
        }
        if o.hide && name == "img" && is_pixel(&c) {
            dom.remove_from_parent(&c);
            st.pixels += 1;
            continue;
        }
        if name == "a" && o.links != LinkMode::Footnotes && cell_text(&c).is_empty() && !has_alt_img(&c) {
            dom.append(&c, NodeOrText::AppendText(StrTendril::from("[link]")));
        }
        let layout = name == "table" && is_layout(&c);
        clean(dom, &c, o, width, st);
        if name == "table" {
            if o.flatten && layout {
                let outer = div(dom);
                for r in rows(&c) {
                    let cs = cells(&r);
                    for cell in cs {
                        let d = div(dom);
                        let moved: Vec<Handle> = cell.children.borrow().clone();
                        for m in moved {
                            dom.remove_from_parent(&m);
                            dom.append(&d, NodeOrText::AppendNode(m));
                        }
                        dom.append(&outer, NodeOrText::AppendNode(d));
                    }
                }
                dom.append_before_sibling(&c, NodeOrText::AppendNode(outer));
                dom.remove_from_parent(&c);
                st.flattened += 1;
            } else if o.stack && !fits(&c, width) {
                stack(dom, &c);
                st.stacked += 1;
            } else {
                st.data_tables += 1;
            }
        }
    }
}

fn cell_text(h: &Handle) -> String {
    let mut b = Vec::new();
    let _ = h.serialize(&mut b);
    let raw = String::from_utf8_lossy(&b);
    let mut out = String::new();
    let mut in_tag = false;
    for ch in raw.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    let out = out.replace("&nbsp;", " ").replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">");
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

// Columns stay only when every cell fits on one line at this width.
fn fits(table: &Handle, width: usize) -> bool {
    let mut cols: Vec<usize> = Vec::new();
    for r in rows(table) {
        for (i, c) in cells(&r).iter().enumerate() {
            let w = cell_text(c).chars().count();
            if cols.len() <= i {
                cols.push(0);
            }
            cols[i] = cols[i].max(w);
        }
    }
    cols.iter().sum::<usize>() + 2 * cols.len().saturating_sub(1) <= width
}

fn el(dom: &RcDom, name: &str) -> Handle {
    dom.create_element(QualName::new(None, ns!(html), LocalName::from(name)), vec![], Default::default())
}

fn stack(dom: &RcDom, table: &Handle) {
    let rs = rows(table);
    let header: Option<Vec<String>> = rs
        .first()
        .filter(|r| cells(r).iter().all(|c| tag(c) == Some("th")))
        .map(|r| cells(r).iter().map(cell_text).collect());
    let outer = el(dom, "div");
    for r in rs.iter().skip(usize::from(header.is_some())) {
        let p = el(dom, "p");
        for (i, cell) in cells(r).iter().enumerate() {
            if cell_text(cell).is_empty() && !has_desc(cell, "a") {
                continue;
            }
            let d = el(dom, "div");
            if let Some(h) = header.as_ref().and_then(|h| h.get(i)).filter(|h| !h.is_empty()) {
                let b = el(dom, "b");
                dom.append(&b, NodeOrText::AppendText(StrTendril::from(format!("{h}: "))));
                dom.append(&d, NodeOrText::AppendNode(b));
            }
            let moved: Vec<Handle> = cell.children.borrow().clone();
            for m in moved {
                dom.remove_from_parent(&m);
                dom.append(&d, NodeOrText::AppendNode(m));
            }
            dom.append(&p, NodeOrText::AppendNode(d));
        }
        dom.append(&outer, NodeOrText::AppendNode(p));
    }
    dom.append_before_sibling(table, NodeOrText::AppendNode(outer));
    dom.remove_from_parent(table);
}

// ---------- render ----------

type Row = Vec<(String, Vec<Ann>)>;

struct Rendered {
    lines: Vec<Row>,
    footnotes: Vec<String>,
    snippet: String,
    ms: f64,
    stats: PassStats,
    err: Option<String>,
}

const JUNK: [char; 6] = ['\u{200c}', '\u{034f}', '\u{00ad}', '\u{200b}', '\u{feff}', '\u{2007}'];

fn render(m: &Msg, width: usize, o: Opts) -> Rendered {
    let t0 = Instant::now();
    let mut stats = PassStats::default();
    let st = Rc::new(RefCell::new(Notes::default()));
    let res: Result<Vec<Row>, String> = match (o.view, &m.html, &m.text) {
        (View::RawHtml, Some(h), _) => Ok(h.lines().map(|l| vec![(l.to_string(), vec![])]).collect()),
        (View::Plain, _, Some(t)) | (View::Rendered, None, Some(t)) => Ok(wrap_plain(t, width)),
        (View::Rendered, Some(h), _) => {
            let deco = Deco { st: st.clone(), html_colours: o.colour == ColourMode::Html, markers: o.links != LinkMode::Inline };
            let html = h.clone();
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut cfg = config::with_decorator(deco)
                    .empty_img_mode(ImageRenderMode::IgnoreEmpty)
                    .allow_width_overflow()
                    .link_footnotes(false);
                if o.css {
                    cfg = cfg.use_doc_css();
                }
                if !o.borders {
                    cfg = cfg.no_table_borders();
                }
                let dom = cfg.parse_html(html.as_bytes()).map_err(|e| e.to_string())?;
                clean(&dom, &dom.document, o, width, &mut stats);
                let tree = cfg.dom_to_render_tree(&dom).map_err(|e| e.to_string())?;
                let lines = cfg.render_to_lines(tree, width).map_err(|e| e.to_string())?;
                Ok::<_, String>(
                    lines
                        .into_iter()
                        .map(|l| {
                            l.iter()
                                .filter_map(|e| match e {
                                    TaggedLineElement::Str(ts) => Some(split_marker(&ts.s, &ts.tag)),
                                    _ => None,
                                })
                                .flatten()
                                .collect()
                        })
                        .collect(),
                )
            }));
            r.unwrap_or_else(|_| Err("converter panicked".into()))
        }
        _ => Ok(vec![vec![("(no such part)".into(), vec![])]]),
    };
    let (lines, err) = match res {
        Ok(l) => (post(l), None),
        Err(e) => (Vec::new(), Some(e)),
    };
    let snippet = snippet(&lines);
    let footnotes = std::mem::take(&mut st.borrow_mut().urls);
    Rendered { lines, footnotes, snippet, ms: t0.elapsed().as_secs_f64() * 1e3, stats, err }
}

fn wrap_plain(t: &str, width: usize) -> Vec<Row> {
    let mut out = Vec::new();
    for l in t.lines() {
        let prefix: String = l.chars().take_while(|c| *c == '>' || *c == ' ').collect();
        let mut cur = prefix.clone();
        for w in l[prefix.len()..].split(' ') {
            if cur.chars().count() + w.chars().count() + 1 > width && cur.len() > prefix.len() {
                out.push(vec![(cur.trim_end().to_string(), vec![])]);
                cur = prefix.clone();
            }
            cur.push_str(w);
            cur.push(' ');
        }
        out.push(vec![(cur.trim_end().to_string(), vec![])]);
    }
    out
}

// Link-end markers arrive glued to link text; split so they style separately.
fn split_marker(s: &str, tag: &[Ann]) -> Vec<(String, Vec<Ann>)> {
    match s.find('\u{e000}') {
        Some(i) => {
            let mut v = Vec::new();
            if i > 0 {
                v.push((s[..i].to_string(), tag.to_vec()));
            }
            let rest = &s[i + 3..];
            let end = rest.find(']').map_or(rest.len(), |j| j + 1);
            v.push((rest[..end].to_string(), vec![Ann::Marker]));
            v.extend(split_marker(&rest[end..], tag));
            v
        }
        None if s.is_empty() => vec![],
        None => vec![(s.to_string(), tag.to_vec())],
    }
}

fn post(lines: Vec<Row>) -> Vec<Row> {
    let mut out: Vec<Row> = Vec::new();
    for mut l in lines {
        for (s, _) in &mut l {
            if s.contains(JUNK) || s.contains('\u{a0}') {
                *s = s.chars().filter(|c| !JUNK.contains(c)).map(|c| if c == '\u{a0}' { ' ' } else { c }).collect();
            }
        }
        while l.last().is_some_and(|(s, _)| s.trim_end().is_empty()) {
            l.pop();
        }
        if let Some((s, _)) = l.last_mut() {
            let t = s.trim_end().len();
            s.truncate(t);
        }
        let blank = l.is_empty();
        if blank && (out.is_empty() || out.last().is_some_and(Vec::is_empty)) {
            continue;
        }
        out.push(l);
    }
    while out.last().is_some_and(Vec::is_empty) {
        out.pop();
    }
    out
}

fn snippet(lines: &[Row]) -> String {
    let mut s = String::new();
    for l in lines {
        let line: String = l
            .iter()
            .filter(|(_, a)| !a.contains(&Ann::Marker) && !a.contains(&Ann::Image))
            .map(|(t, _)| t.as_str())
            .collect();
        {
            for w in line.split_whitespace() {
                if w != "[link]" && !w.chars().all(|c| "─│>•".contains(c)) {
                    s.push_str(w);
                    s.push(' ');
                }
            }
        }
        if s.chars().count() > 200 {
            break;
        }
    }
    s.chars().take(200).collect()
}

// ---------- styling ----------

fn style(anns: &[Ann], mode: ColourMode) -> Style {
    let mut st = Style::default();
    if mode == ColourMode::Off {
        return st;
    }
    for a in anns {
        st = match a {
            Ann::Link(_) => st.fg(Color::Blue).add_modifier(Modifier::UNDERLINED),
            Ann::Marker => st.fg(Color::DarkGray),
            Ann::Image => st.fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
            Ann::Em => st.add_modifier(Modifier::ITALIC),
            Ann::Strong => st.add_modifier(Modifier::BOLD),
            Ann::Code | Ann::Pre => st.fg(Color::Yellow),
            Ann::Colour(r, g, b) => st.fg(Color::Rgb(*r, *g, *b)),
            Ann::None => st,
        };
    }
    st
}

fn ansi(st: Style) -> String {
    let mut codes = Vec::new();
    if st.add_modifier.contains(Modifier::BOLD) {
        codes.push("1".to_string());
    }
    if st.add_modifier.contains(Modifier::ITALIC) {
        codes.push("3".into());
    }
    if st.add_modifier.contains(Modifier::UNDERLINED) {
        codes.push("4".into());
    }
    match st.fg {
        Some(Color::Blue) => codes.push("34".into()),
        Some(Color::DarkGray) => codes.push("90".into()),
        Some(Color::Yellow) => codes.push("33".into()),
        Some(Color::Rgb(r, g, b)) => codes.push(format!("38;2;{r};{g};{b}")),
        _ => {}
    }
    if codes.is_empty() {
        String::new()
    } else {
        format!("\x1b[{}m", codes.join(";"))
    }
}

fn header_lines(m: &Msg) -> Vec<(&'static str, String)> {
    vec![("From", m.from.clone()), ("Subject", m.subject.clone()), ("Date", m.date.clone())]
}

fn as_text(m: &Msg, r: &Rendered, o: Opts) -> String {
    let mut out = String::new();
    for (k, v) in header_lines(m) {
        if o.pager_ansi {
            out.push_str(&format!("\x1b[1m{k}:\x1b[0m {v}\n"));
        } else {
            out.push_str(&format!("{k}: {v}\n"));
        }
    }
    out.push('\n');
    for l in &r.lines {
        for (s, a) in l {
            let code = if o.pager_ansi { ansi(style(a, o.colour)) } else { String::new() };
            let url = link_url(a, r).filter(|_| o.pager_ansi && o.links != LinkMode::Footnotes);
            if let Some(u) = &url {
                out.push_str(&format!("\x1b]8;;{u}\x1b\\"));
            }
            if code.is_empty() {
                out.push_str(s);
            } else {
                out.push_str(&format!("{code}{s}\x1b[0m"));
            }
            if url.is_some() {
                out.push_str("\x1b]8;;\x1b\\");
            }
        }
        out.push('\n');
    }
    if !r.footnotes.is_empty() && o.links != LinkMode::Inline {
        out.push('\n');
        for (i, u) in r.footnotes.iter().enumerate() {
            if o.pager_ansi {
                out.push_str(&format!("\x1b[90m[{}]\x1b[0m {u}\n", i + 1));
            } else {
                out.push_str(&format!("[{}] {u}\n", i + 1));
            }
        }
    }
    out
}

fn link_url(a: &[Ann], r: &Rendered) -> Option<String> {
    a.iter().rev().find_map(|x| match x {
        Ann::Link(n) if *n > 0 => r.footnotes.get(n - 1).map(|u| {
            if u.contains('@') && !u.contains("://") { format!("mailto:{u}") } else { u.clone() }
        }),
        _ => None,
    })
}

// Screen run of link text to re-print wrapped in OSC 8 after ratatui draws.
struct Hyper {
    x: u16,
    y: u16,
    text: String,
    style: Style,
    url: String,
}

// ---------- app ----------

struct App {
    msgs: Vec<Msg>,
    sel: usize,
    off: usize,
    scroll: usize,
    o: Opts,
    help: bool,
    cache: Option<(usize, usize, Opts, Rc<Rendered>)>,
    toast: Option<String>,
    hypers: Vec<Hyper>,
    hint: Option<String>,
    targets: Vec<(String, String)>,
}

impl App {
    fn rendered(&mut self, width: usize) -> Rc<Rendered> {
        let w = self.o.width.unwrap_or(width);
        if let Some((i, cw, co, r)) = &self.cache {
            if *i == self.sel && *cw == w && *co == self.o {
                return r.clone();
            }
        }
        let r = Rc::new(render(&self.msgs[self.sel], w, self.o));
        self.cache = Some((self.sel, w, self.o, r.clone()));
        r
    }
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let dir = PathBuf::from(args.get(1).expect("usage: preview-render <dir-of-eml> [--dump <out>]"));
    let msgs = load(&dir);
    if msgs.is_empty() {
        eprintln!("no .eml in {}", dir.display());
        return Ok(());
    }
    if args.get(2).map(String::as_str) == Some("--dump") {
        let out = PathBuf::from(args.get(3).expect("--dump <out>"));
        std::fs::create_dir_all(&out)?;
        let inline = std::env::var("DUMP_INLINE").is_ok();
        let o = Opts { pager_ansi: inline, links: if inline { LinkMode::Inline } else { LinkMode::Footnotes }, ..Opts::default() };
        for m in &msgs {
            let r = render(m, 80, o);
            let mut body = as_text(m, &r, o);
            body.push_str(&format!(
                "\n---\nsnippet: {}\nms: {:.2} hidden: {} pixels: {} flattened: {} data tables: {} stacked: {} err: {:?}\n",
                r.snippet, r.ms, r.stats.hidden, r.stats.pixels, r.stats.flattened, r.stats.data_tables, r.stats.stacked, r.err
            ));
            std::fs::write(out.join(format!("{}.txt", m.name)), body)?;
        }
        println!("dumped {} messages to {}", msgs.len(), out.display());
        return Ok(());
    }
    let mut app = App { msgs, sel: 0, off: 0, scroll: 0, o: Opts::default(), help: false, cache: None, toast: None, hypers: Vec::new(), hint: None, targets: Vec::new() };
    let mut term = ratatui::init();
    let res = run(&mut term, &mut app);
    ratatui::restore();
    res
}

fn run(term: &mut DefaultTerminal, app: &mut App) -> io::Result<()> {
    let mut preview_w = 80;
    loop {
        term.draw(|f| preview_w = draw(f, app))?;
        if app.o.links != LinkMode::Footnotes && app.hint.is_none() {
            write_hypers(&app.hypers)?;
        }
        let Event::Key(k) = event::read()? else { continue };
        if k.kind != KeyEventKind::Press {
            continue;
        }
        app.toast = None;
        let n = app.msgs.len();
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        if app.help {
            app.help = false;
            continue;
        }
        if let Some(prefix) = app.hint.as_mut() {
            match k.code {
                KeyCode::Char(c) => {
                    prefix.push(c);
                    let p = prefix.clone();
                    if let Some((_, url)) = app.targets.iter().find(|(l, _)| *l == p) {
                        let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
                        let _ = Command::new(opener).arg(url).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
                        app.toast = Some(format!(" opened {url}"));
                        app.hint = None;
                    } else if !app.targets.iter().any(|(l, _)| l.starts_with(&p)) {
                        app.hint = None;
                    }
                }
                _ => app.hint = None,
            }
            continue;
        }
        match k.code {
            KeyCode::Char('q') => return Ok(()),
            KeyCode::Char('j') | KeyCode::Down => {
                app.sel = (app.sel + 1).min(n - 1);
                app.scroll = 0;
            }
            KeyCode::Char('k') | KeyCode::Up => {
                app.sel = app.sel.saturating_sub(1);
                app.scroll = 0;
            }
            KeyCode::Char('g') => (app.sel, app.scroll) = (0, 0),
            KeyCode::Char('G') => (app.sel, app.scroll) = (n - 1, 0),
            KeyCode::Char('d') if ctrl => app.scroll += 15,
            KeyCode::Char('u') if ctrl => app.scroll = app.scroll.saturating_sub(15),
            KeyCode::Char('?') => app.help = true,
            KeyCode::Char('c') => app.o.css = !app.o.css,
            KeyCode::Char('h') => app.o.hide = !app.o.hide,
            KeyCode::Char('f') => app.o.flatten = !app.o.flatten,
            KeyCode::Char('b') => app.o.borders = !app.o.borders,
            KeyCode::Char('s') => app.o.stack = !app.o.stack,
            KeyCode::Char('a') => app.o.pager_ansi = !app.o.pager_ansi,
            KeyCode::Char('o') if !app.targets.is_empty() => app.hint = Some(String::new()),
            KeyCode::Char('L') => {
                app.o.links = match app.o.links {
                    LinkMode::Inline => LinkMode::Footnotes,
                    LinkMode::Footnotes => LinkMode::Both,
                    LinkMode::Both => LinkMode::Inline,
                }
            }
            KeyCode::Char('m') => {
                app.o.colour = match app.o.colour {
                    ColourMode::Semantic => ColourMode::Html,
                    ColourMode::Html => ColourMode::Off,
                    ColourMode::Off => ColourMode::Semantic,
                }
            }
            KeyCode::Char('v') => {
                app.o.view = match app.o.view {
                    View::Rendered => View::Plain,
                    View::Plain => View::RawHtml,
                    View::RawHtml => View::Rendered,
                }
            }
            KeyCode::Char('w') => {
                app.o.width = match app.o.width {
                    None => Some(40),
                    Some(40) => Some(60),
                    Some(60) => Some(80),
                    Some(80) => Some(100),
                    _ => None,
                }
            }
            KeyCode::Enter | KeyCode::Char('l') => {
                let size = term.size()?;
                let w = app.o.width.unwrap_or((size.width as usize).min(100) - 2);
                let r = render(&app.msgs[app.sel], w, app.o);
                let text = as_text(&app.msgs[app.sel], &r, app.o);
                ratatui::restore();
                let pager = std::env::var("PAGER").unwrap_or_else(|_| "less".into());
                let mut cmd = Command::new(&pager);
                if pager == "less" && app.o.pager_ansi {
                    cmd.arg("-R");
                }
                if let Ok(mut child) = cmd.stdin(Stdio::piped()).spawn() {
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = stdin.write_all(text.as_bytes());
                    }
                    let _ = child.wait();
                }
                *term = ratatui::init();
                let _ = preview_w;
            }
            _ => {}
        }
    }
}

// Equal-length labels so no label is a prefix of another.
fn hint_labels(n: usize) -> Vec<String> {
    const ONE: &str = "asdfghjklqwertyuiopzxcvbnm";
    const HOME: &str = "asdfghjkl";
    if n <= ONE.len() {
        ONE.chars().take(n).map(String::from).collect()
    } else {
        HOME.chars().flat_map(|a| HOME.chars().map(move |b| format!("{a}{b}"))).take(n).collect()
    }
}

fn write_hypers(hs: &[Hyper]) -> io::Result<()> {
    use crossterm::{cursor::MoveTo, queue, style::Print};
    let mut out = io::stdout();
    for h in hs {
        let code = ansi(h.style);
        queue!(out, MoveTo(h.x, h.y), Print(format!("\x1b]8;;{}\x1b\\{code}{}\x1b[0m\x1b]8;;\x1b\\", h.url, h.text)))?;
    }
    out.flush()
}

fn draw(f: &mut Frame, app: &mut App) -> usize {
    let area = f.area();
    let [top, main, bottom] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(1), Constraint::Length(1)]).areas(area);
    let o = app.o;
    let on = |b: bool| if b { "on" } else { "off" };
    f.render_widget(
        Paragraph::new(format!(
            " PROTOTYPE  L links:{:?}  c css:{}  h hide:{}  f flatten:{}  b borders:{}  s stack:{}  m colour:{:?}  v view:{:?}  w width:{}  a pager-ansi:{}",
            o.links,
            on(o.css),
            on(o.hide),
            on(o.flatten),
            on(o.borders),
            on(o.stack),
            o.colour,
            o.view,
            o.width.map_or("pane".into(), |w| w.to_string()),
            on(o.pager_ansi)
        ))
        .style(Style::default().bg(Color::Magenta).fg(Color::Black)),
        top,
    );
    let [list_a, prev_a] = Layout::horizontal([Constraint::Percentage(35), Constraint::Percentage(65)]).areas(main);

    // list
    let rows_visible = (list_a.height.saturating_sub(2) / 3).max(1) as usize;
    if app.sel < app.off {
        app.off = app.sel;
    }
    if app.sel >= app.off + rows_visible {
        app.off = app.sel + 1 - rows_visible;
    }
    let lw = list_a.width.saturating_sub(2) as usize;
    let sel_snip = app.rendered(prev_a.width.saturating_sub(4) as usize).snippet.clone();
    let mut lines = Vec::new();
    for (i, m) in app.msgs.iter().enumerate().skip(app.off).take(rows_visible) {
        let sel = i == app.sel;
        let base = if sel { Style::default().bg(Color::DarkGray) } else { Style::default() };
        let pad = |s: String| format!("{:<lw$}", s.chars().take(lw).collect::<String>());
        let snip = if sel { sel_snip.clone() } else { String::new() };
        lines.push(Line::styled(pad(m.from.clone()), base.add_modifier(Modifier::BOLD)));
        lines.push(Line::styled(pad(m.subject.clone()), base));
        lines.push(Line::styled(pad(if sel { snip } else { m.name.clone() }), base.fg(Color::Gray)));
    }
    f.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(" Corpus ")),
        list_a,
    );

    // preview
    let pw = prev_a.width.saturating_sub(4) as usize;
    let r = app.rendered(pw);
    let m = &app.msgs[app.sel];
    let mut pl: Vec<Line> = header_lines(m)
        .into_iter()
        .map(|(k, v)| Line::from(vec![Span::styled(format!("{k}: "), Style::default().add_modifier(Modifier::BOLD)), Span::raw(v)]))
        .collect();
    pl.push(Line::raw(""));
    if let Some(e) = &r.err {
        pl.push(Line::styled(format!("render error: {e}"), Style::default().fg(Color::Red)));
    }
    let body_start = pl.len();
    let mut order: Vec<String> = Vec::new();
    for l in &r.lines {
        for (_, a) in l {
            if let Some(u) = link_url(a, &r) {
                if !order.contains(&u) {
                    order.push(u);
                }
            }
        }
    }
    app.targets = hint_labels(order.len()).into_iter().zip(order).collect();
    let label_style = Style::default().bg(Color::Yellow).fg(Color::Black).add_modifier(Modifier::BOLD);
    let mut prev_url: Option<String> = None;
    for l in &r.lines {
        let mut spans = Vec::new();
        for (s, a) in l {
            let url = link_url(a, &r);
            if let (Some(prefix), Some(u)) = (&app.hint, &url) {
                if prev_url.as_ref() != Some(u) {
                    if let Some((lbl, _)) = app.targets.iter().find(|(_, t)| t == u) {
                        if lbl.starts_with(prefix.as_str()) {
                            spans.push(Span::styled(format!("[{lbl}]"), label_style));
                        }
                    }
                }
            }
            if !s.trim().is_empty() || url.is_none() {
                prev_url = url;
            }
            spans.push(Span::styled(s.clone(), style(a, o.colour)));
        }
        pl.push(Line::from(spans));
    }
    app.hypers.clear();
    {
        let (ix, iy) = (prev_a.x + 2, prev_a.y + 1);
        let ih = prev_a.height.saturating_sub(2) as usize;
        for (li, l) in r.lines.iter().enumerate() {
            let row = body_start + li;
            if row < app.scroll || row - app.scroll >= ih {
                continue;
            }
            let mut x = 0usize;
            for (s, a) in l {
                let w = s.chars().count();
                if let Some(url) = link_url(a, &r) {
                    if x + w <= pw {
                        app.hypers.push(Hyper { x: ix + x as u16, y: iy + (row - app.scroll) as u16, text: s.clone(), style: style(a, o.colour), url });
                    }
                }
                x += w;
            }
        }
    }
    if !r.footnotes.is_empty() && o.links != LinkMode::Inline {
        pl.push(Line::raw(""));
        for (i, u) in r.footnotes.iter().enumerate() {
            let mark = Style::default().fg(if o.colour == ColourMode::Off { Color::Reset } else { Color::DarkGray });
            let short = u.split_once("://").map_or(u.as_str(), |(_, r)| r);
            let room = pw.saturating_sub(format!("[{}] ", i + 1).len());
            let shown: String = if short.chars().count() > room {
                short.chars().take(room.saturating_sub(1)).chain(['…']).collect()
            } else {
                short.to_string()
            };
            pl.push(Line::from(vec![Span::styled(format!("[{}] ", i + 1), mark), Span::raw(shown)]));
        }
    }
    let max_scroll = pl.len().saturating_sub(1);
    app.scroll = app.scroll.min(max_scroll);
    f.render_widget(
        Paragraph::new(pl)
            .scroll((app.scroll as u16, 0))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" {}/{} ", app.sel + 1, app.msgs.len()))
                    .padding(Padding::horizontal(1)),
            ),
        prev_a,
    );

    let s = r.stats;
    f.render_widget(
        Paragraph::new(format!(
            " {:.2} ms  hidden:{} pixels:{} flattened:{} data-tables:{} links:{}  html:{} text:{}   j/k Ctrl-d/u Enter=pager ?",
            r.ms,
            s.hidden,
            s.pixels,
            s.flattened,
            s.data_tables,
            r.footnotes.len(),
            m.html.is_some(),
            m.text.is_some()
        ))
        .style(Style::default().fg(Color::Gray)),
        bottom,
    );
    if app.hint.is_some() {
        f.render_widget(Clear, bottom);
        f.render_widget(Paragraph::new(" -- open link: type its label, any other key cancels --").style(Style::default().fg(Color::Yellow)), bottom);
    }
    if let Some(t) = &app.toast {
        f.render_widget(Clear, bottom);
        f.render_widget(Paragraph::new(t.as_str()), bottom);
    }
    if app.help {
        let w = 64.min(area.width);
        let h = 19.min(area.height);
        let r = ratatui::layout::Rect { x: (area.width - w) / 2, y: (area.height - h) / 2, width: w, height: h };
        f.render_widget(Clear, r);
        f.render_widget(
            Paragraph::new(
                "j/k g/G   move\nCtrl-d/u  scroll Preview\nEnter/l   pipe to $PAGER\nq         quit\n\nc  doc CSS (display:none)\nh  hidden-style + pixel removal\nf  flatten layout tables\no  open a link by label (hint mode)\nL  links: Inline (OSC 8) / Footnotes / Both\nb  table borders\ns  stack data tables that do not fit\nm  colour: Semantic / Html / Off\nv  view: Rendered / text/plain / raw HTML\nw  width: pane / 40 / 60 / 80 / 100\na  pager ANSI on/off",
            )
            .block(Block::default().borders(Borders::ALL).title(" ? ")),
            r,
        );
    }
    pw
}
