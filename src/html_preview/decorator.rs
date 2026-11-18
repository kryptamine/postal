use std::sync::Arc;

use html2text::render::{TaggedLine, TextDecorator};

use crate::marks::Marks;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Annotation {
    #[default]
    Plain,
    Strong,
    Emphasis,
    Link(Arc<str>),
    Code,
    Alt,
}

/// Marks text by meaning and adds no markup of its own: an image is its alt
/// text in brackets, and each footnoted URL is listed as `[n] url`.
#[derive(Clone, Copy)]
pub struct Decorator;

/// The marks of text inside every one of `annotations`.
pub fn marks(annotations: &[Annotation]) -> Marks {
    annotations
        .iter()
        .fold(Marks::default(), |marks, annotation| {
            marks.with(match annotation {
                Annotation::Plain => Marks::default(),
                Annotation::Strong => Marks::STRONG,
                Annotation::Emphasis => Marks::EMPHASIS,
                Annotation::Link(_) => Marks::LINK,
                Annotation::Code => Marks::CODE,
                Annotation::Alt => Marks::ALT,
            })
        })
}

/// The URL of the innermost link around text inside `annotations`.
pub fn link(annotations: &[Annotation]) -> Option<&Arc<str>> {
    annotations
        .iter()
        .rev()
        .find_map(|annotation| match annotation {
            Annotation::Link(url) => Some(url),
            _ => None,
        })
}

impl TextDecorator for Decorator {
    type Annotation = Annotation;

    fn decorate_link_start(&mut self, url: &str) -> (String, Annotation) {
        (String::new(), Annotation::Link(Arc::from(url)))
    }

    fn decorate_link_end(&mut self) -> String {
        String::new()
    }

    fn decorate_em_start(&self) -> (String, Annotation) {
        (String::new(), Annotation::Emphasis)
    }

    fn decorate_em_end(&self) -> String {
        String::new()
    }

    fn decorate_strong_start(&self) -> (String, Annotation) {
        (String::new(), Annotation::Strong)
    }

    fn decorate_strong_end(&self) -> String {
        String::new()
    }

    fn decorate_strikeout_start(&self) -> (String, Annotation) {
        (String::new(), Annotation::Plain)
    }

    fn decorate_strikeout_end(&self) -> String {
        String::new()
    }

    fn decorate_code_start(&self) -> (String, Annotation) {
        (String::new(), Annotation::Code)
    }

    fn decorate_code_end(&self) -> String {
        String::new()
    }

    fn decorate_preformat_first(&self) -> Annotation {
        Annotation::Code
    }

    fn decorate_preformat_cont(&self) -> Annotation {
        Annotation::Code
    }

    fn decorate_image(&mut self, _src: &str, title: &str) -> (String, Annotation) {
        (format!("[{}]", title.trim()), Annotation::Alt)
    }

    fn header_prefix(&self, _level: usize) -> String {
        String::new()
    }

    fn quote_prefix(&self) -> String {
        "> ".to_owned()
    }

    fn unordered_item_prefix(&self) -> String {
        "• ".to_owned()
    }

    fn ordered_item_prefix(&self, i: i64) -> String {
        format!("{i}. ")
    }

    fn make_subblock_decorator(&self) -> Self {
        *self
    }

    fn finalise(&mut self, urls: Vec<String>) -> Vec<TaggedLine<Annotation>> {
        urls.iter()
            .enumerate()
            .map(|(i, url)| {
                TaggedLine::from_string(format!("[{}] {url}", i + 1), &Annotation::Plain)
            })
            .collect()
    }
}
