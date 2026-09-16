//! `axuielement` embeds Swift code that needs Swift's back-deployment
//! compatibility static libraries at link time. `swiftc` adds their
//! directory to the linker search path automatically; plain `cargo`/`clang`
//! do not, so a Command-Line-Tools-only install (no full Xcode.app) fails to
//! link with undefined `__swift_FORCE_LOAD_$_swiftCompatibility*` symbols
//! even though the libraries are present on disk.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

    let Some(developer_dir) = active_developer_dir() else {
        return;
    };

    // Covers both a Command Line Tools-only install and a full Xcode.app
    // install, whose Swift lib directory lives one level deeper.
    let candidates = [
        format!("{developer_dir}/usr/lib/swift/macosx"),
        format!("{developer_dir}/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift/macosx"),
    ];

    for candidate in candidates {
        if std::path::Path::new(&candidate).is_dir() {
            println!("cargo:rustc-link-search=native={candidate}");
        }
    }
}

fn active_developer_dir() -> Option<String> {
    if let Ok(dir) = std::env::var("DEVELOPER_DIR")
        && !dir.is_empty()
    {
        return Some(dir);
    }

    let output = std::process::Command::new("xcode-select")
        .arg("-p")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let dir = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if dir.is_empty() { None } else { Some(dir) }
}
