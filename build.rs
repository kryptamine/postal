//! Compiles the vendored litehtml, its bundled gumbo and the cell-grid
//! container in `cpp/`, for the rendered view of HTML mail, names the
//! target, whose release archive `postal update` fetches, and takes in the
//! macOS mailto: launcher.

use std::path::{Path, PathBuf};

#[expect(clippy::panic, reason = "a build script fails by panicking")]
fn sources(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("reading {}: {error}", dir.display()))
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|e| e == extension))
        .collect();
    files.sort();
    files
}

#[expect(clippy::panic, reason = "a build script fails by panicking")]
fn target() -> String {
    std::env::var("TARGET").unwrap_or_else(|error| panic!("TARGET: {error}"))
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
    println!("cargo:rustc-env=POSTAL_TARGET={}", target());
    launcher();
}

/// Copies the launcher zip `POSTAL_LAUNCHER` names to
/// `$OUT_DIR/launcher.zip`, or writes it empty when the variable is unset.
#[expect(clippy::panic, reason = "a build script fails by panicking")]
fn launcher() {
    println!("cargo:rerun-if-env-changed=POSTAL_LAUNCHER");
    let out = std::env::var_os("OUT_DIR").unwrap_or_else(|| panic!("OUT_DIR is not set"));
    let out = Path::new(&out).join("launcher.zip");
    let copied = match std::env::var("POSTAL_LAUNCHER") {
        Ok(zip) if !zip.is_empty() => {
            println!("cargo:rerun-if-changed={zip}");
            std::fs::copy(&zip, &out).map(drop)
        }
        _ => std::fs::write(&out, []),
    };

    copied.unwrap_or_else(|error| panic!("{}: {error}", out.display()));
}
