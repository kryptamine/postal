//! The layer direction and the import invariants of ARCHITECTURE.md, read
//! from the paths each file of `src/` names; see ADR 0038.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use syn::{
    ItemUse, UseTree,
    visit::{self, Visit},
};

/// Layers in dependency order from ARCHITECTURE.md, lowest first.
const LAYERS: &[&str] = &[
    "domain", "cache", "account", "render", "compose", "mail", "ui", "cli", "app",
];

enum Allowed {
    Only(&'static [&'static str]),
    Except(&'static [&'static str]),
}

impl Allowed {
    fn admits(&self, layer: &str) -> bool {
        match *self {
            Self::Only(owners) => owners.contains(&layer),
            Self::Except(barred) => !barred.contains(&layer),
        }
    }
}

/// Modules of `ui` that draw only, so they name nothing else of `ui`.
const UI_LEAVES: &[&str] = &["widgets"];

/// Layers that never touch the network.
const OFFLINE: &[&str] = &["domain", "cache", "render"];

/// The Invariants of ARCHITECTURE.md as the paths a layer may name.
const BOUNDARIES: &[(&str, Allowed)] = &[
    ("rusqlite", Allowed::Only(&["cache"])),
    ("ratatui", Allowed::Only(&["ui"])),
    ("crossterm", Allowed::Only(&["ui"])),
    ("async_imap", Allowed::Only(&["mail"])),
    ("tokio_rustls", Allowed::Only(&["mail"])),
    ("lettre::transport", Allowed::Only(&["mail"])),
    ("lettre::Transport", Allowed::Only(&["mail"])),
    ("lettre::AsyncTransport", Allowed::Only(&["mail"])),
    ("lettre::SmtpTransport", Allowed::Only(&["mail"])),
    ("lettre::AsyncSmtpTransport", Allowed::Only(&["mail"])),
    ("rmcp", Allowed::Only(&["cli"])),
    ("tokio", Allowed::Except(&["domain"])),
    ("std::env", Allowed::Except(&["domain"])),
    ("std::io", Allowed::Except(&["domain"])),
    ("std::process", Allowed::Except(&["domain"])),
    ("std::fs", Allowed::Except(&["domain", "render"])),
    ("std::net", Allowed::Except(OFFLINE)),
    ("tokio::net", Allowed::Except(OFFLINE)),
    ("ureq", Allowed::Except(OFFLINE)),
];

// Protects the layer direction and crate boundaries documented in ARCHITECTURE.md.
#[test]
fn layers_respect_architecture_boundaries() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut undocumented = BTreeSet::new();
    let mut violations = Vec::new();

    for path in rust_files(&src) {
        let relative = path.strip_prefix(&src).unwrap();
        let Some(layer) = layer_of(relative) else {
            continue;
        };

        if !LAYERS.contains(&layer) {
            undocumented.insert(format!("undocumented layer directory: src/{layer}"));
            continue;
        }

        let source = fs::read_to_string(&path).unwrap();
        let file = syn::parse_file(&source)
            .unwrap_or_else(|error| panic!("parsing {}: {error}", path.display()));
        let mut paths = NamedPaths::default();
        paths.visit_file(&file);

        let leaf = ui_leaf_of(relative);
        let file = relative.display();
        violations.extend(
            paths
                .0
                .iter()
                .filter_map(|named| {
                    violation(layer, named)
                        .or_else(|| leaf.and_then(|leaf| leaf_violation(leaf, named)))
                })
                .map(|violation| format!("{file}: {violation}")),
        );
    }

    violations.extend(undocumented);
    assert!(
        violations.is_empty(),
        "architecture violations:\n{}",
        violations.join("\n")
    );
}

fn violation(layer: &str, named: &str) -> Option<String> {
    if let Some(target) = named
        .strip_prefix("crate::")
        .and_then(|rest| rest.split("::").next())
        && LAYERS.contains(&target)
        && !may_import(layer, target)
    {
        return Some(format!("layer `{layer}` must not import `{target}`"));
    }

    BOUNDARIES
        .iter()
        .find(|(path, allowed)| starts_with_path(named, path) && !allowed.admits(layer))
        .map(|(path, _)| format!("`{path}` is not allowed in `{layer}`"))
}

fn leaf_violation(leaf: &str, named: &str) -> Option<String> {
    let target = named.strip_prefix("crate::ui::")?.split("::").next()?;
    (target != leaf).then(|| format!("`ui::{leaf}` must not import `ui::{target}`"))
}

/// The leaf of `UI_LEAVES` a file of `src/` is in, if any.
fn ui_leaf_of(relative: &Path) -> Option<&'static str> {
    let mut components = relative.components();
    (components.next()?.as_os_str() == "ui").then_some(())?;
    let group = components.next()?.as_os_str();
    UI_LEAVES.iter().copied().find(|&leaf| group == leaf)
}

/// `ui` and `cli` are peers; `app` may import any layer except `cli`.
fn may_import(from: &str, to: &str) -> bool {
    if matches!((from, to), ("ui" | "app", "cli") | ("cli", "ui")) {
        return false;
    }

    let rank = |layer| LAYERS.iter().position(|&known| known == layer);
    rank(to) <= rank(from)
}

fn starts_with_path(named: &str, prefix: &str) -> bool {
    named
        .strip_prefix(prefix)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with("::"))
}

/// `None` for a file at the root of `src/`, such as `main.rs`.
fn layer_of(relative: &Path) -> Option<&str> {
    let mut components = relative.components();
    let first = components.next()?.as_os_str().to_str()?;
    components.next()?;
    Some(first)
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rust_files(dir, &mut files);
    files.sort();
    files
}

fn collect_rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_rust_files(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
}

/// Every path a file names, in `use` or in code, cut to its first three
/// segments: `crate::ui::frame`, `std::fs`, `rusqlite::Connection`.
#[derive(Default)]
struct NamedPaths(BTreeSet<String>);

impl NamedPaths {
    fn record(&mut self, segments: &[String]) {
        if !segments.is_empty() {
            self.0.insert(segments[..segments.len().min(3)].join("::"));
        }
    }

    fn record_use(&mut self, prefix: &mut Vec<String>, tree: &UseTree) {
        match tree {
            UseTree::Path(path) => {
                prefix.push(path.ident.to_string());
                self.record_use(prefix, &path.tree);
                prefix.pop();
            }
            UseTree::Name(name) => self.record_leaf(prefix, &name.ident),
            UseTree::Rename(rename) => self.record_leaf(prefix, &rename.ident),
            UseTree::Glob(_) => self.record(prefix),
            UseTree::Group(group) => {
                for item in &group.items {
                    self.record_use(prefix, item);
                }
            }
        }
    }

    fn record_leaf(&mut self, prefix: &mut Vec<String>, ident: &syn::Ident) {
        prefix.push(ident.to_string());
        self.record(prefix);
        prefix.pop();
    }
}

impl<'ast> Visit<'ast> for NamedPaths {
    fn visit_item_use(&mut self, node: &'ast ItemUse) {
        self.record_use(&mut Vec::new(), &node.tree);
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        let segments: Vec<String> = path
            .segments
            .iter()
            .take(3)
            .map(|segment| segment.ident.to_string())
            .collect();
        self.record(&segments);
        visit::visit_path(self, path);
    }
}
