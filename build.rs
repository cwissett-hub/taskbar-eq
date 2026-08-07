//! Compiles the Windows resource script, if the toolchain to do so is present.
//!
//! DELIBERATELY OPTIONAL. This project's whole distribution story is "one portable exe, cargo build,
//! no installer", and it has never needed a build script. A build that FAILS because the Windows SDK
//! is missing would be a worse regression than shipping without version metadata - the exe still runs
//! identically either way, it just has no icon in Explorer and no CompanyName. So a missing `rc.exe`
//! is a warning, never an error.
//!
//! `rc.exe` rather than a crate like `embed-resource` or `winres` for the same reason the tray icon is
//! drawn at runtime: neither is in the local cargo registry cache, so adding one would mean a
//! crates.io fetch on a machine whose proxy already blocks `gh`. The SDK is already installed here,
//! and `rc.exe` has a stable command line.

use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=resources/taskbar-eq.rc");
    println!("cargo:rerun-if-changed=assets/taskbar-eq.ico");
    println!("cargo:rerun-if-changed=build.rs");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let Some(rc) = find_rc() else {
        println!(
            "cargo:warning=rc.exe not found, so the exe will have no icon or version metadata. \
             Install the Windows SDK to embed them. The build is otherwise unaffected."
        );
        return;
    };
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("taskbar-eq.res");
    let status = std::process::Command::new(&rc)
        .arg("/nologo")
        .arg("/fo")
        .arg(&out)
        .arg("resources/taskbar-eq.rc")
        .status();
    match status {
        Ok(s) if s.success() && out.exists() => {
            // Handed to the linker directly. A .res on the link line is how a resource gets into a
            // PE; there is no cargo-native way to express it.
            println!("cargo:rustc-link-arg={}", out.display());
        }
        Ok(s) => println!("cargo:warning=rc.exe exited with {s}; building without resources"),
        Err(e) => println!("cargo:warning=could not run rc.exe ({e}); building without resources"),
    }
}

/// The newest x64 `rc.exe` from an installed Windows SDK.
///
/// Sorted by directory name descending so a machine with several SDK versions picks the newest rather
/// than whichever the filesystem happens to return first.
fn find_rc() -> Option<PathBuf> {
    for root in [
        r"C:\Program Files (x86)\Windows Kits\10\bin",
        r"C:\Program Files\Windows Kits\10\bin",
    ] {
        let root = Path::new(root);
        if !root.is_dir() {
            continue;
        }
        let mut versions: Vec<_> = std::fs::read_dir(root)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        versions.sort();
        versions.reverse();
        for v in versions {
            let candidate = v.join("x64").join("rc.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}
