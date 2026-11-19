//! Compiles the vendored litehtml, its bundled gumbo and the cell-grid
//! container in `cpp/`, for the rendered view of HTML mail.

use std::path::{Path, PathBuf};

fn sources(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("reading {}: {error}", dir.display()))
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|e| e == extension))
        .collect();
    files.sort();
    files
}

fn main() {
    let litehtml = Path::new("vendor/litehtml");
    let gumbo = litehtml.join("src/gumbo/src");

    // Each library is linked after the one that needs it, as GNU ld wants.
    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .files(sources(Path::new("cpp"), "cpp"))
        .include(litehtml.join("include"))
        .include("cpp")
        .warnings_into_errors(true)
        .opt_level(2)
        .compile("cell_grid");

    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .files(sources(&litehtml.join("src"), "cpp"))
        .include(litehtml.join("include"))
        .include(litehtml.join("include/litehtml"))
        .include(litehtml.join("src"))
        .include(&gumbo)
        .warnings(false)
        .opt_level(2)
        .compile("litehtml");

    cc::Build::new()
        .files(sources(&gumbo, "c"))
        .include(&gumbo)
        .std("c99")
        .warnings(false)
        .opt_level(2)
        .compile("gumbo");

    println!("cargo:rerun-if-changed=cpp");
    println!("cargo:rerun-if-changed=vendor/litehtml");
    println!("cargo:rerun-if-changed=build.rs");
}
