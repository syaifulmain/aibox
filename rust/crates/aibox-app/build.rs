use std::{env, path::PathBuf, process::Command};

use cxx_qt_build::CxxQtBuilder;

/// Avoid `qt-build-utils` falling back to `-fuse-ld=gold`, which is deprecated
/// in GNU binutils and triggers a compiler warning on every link step. Prefer
/// `lld` or `mold` when available on the host; otherwise keep the standard GNU
/// `ld` (`bfd`) linker by marking `-fuse-ld` as already configured for this
/// build script process.
fn configure_linker() {
    let flags = env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();
    if flags.contains("-fuse-ld") {
        return;
    }

    let has_tool = |cmd: &str| Command::new(cmd).arg("--help").output().is_ok();
    if has_tool("lld") {
        println!("cargo::rustc-link-arg=-fuse-ld=lld");
    } else if has_tool("mold") {
        println!("cargo::rustc-link-arg=-fuse-ld=mold");
    }

    let updated = if flags.is_empty() {
        "-C\x1flink-arg=-fuse-ld=bfd".to_string()
    } else {
        format!("{flags}\x1f-C\x1flink-arg=-fuse-ld=bfd")
    };
    env::set_var("CARGO_ENCODED_RUSTFLAGS", updated);
}

fn main() {
    configure_linker();

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root_dir = manifest_dir
        .join("../../..")
        .canonicalize()
        .expect("repo root");
    let cpp_dir = manifest_dir.join("cpp");

    let cpp_sources = [
        cpp_dir.join("main.cpp"),
        cpp_dir.join("shim/clipboard_shim.cpp"),
        cpp_dir.join("shim/qml_helpers.cpp"),
        cpp_dir.join("shim/test_helpers.cpp"),
        cpp_dir.join("shim/translator_shim.cpp"),
    ];

    let mut builder = CxxQtBuilder::new()
        .qt_module("Gui")
        .qt_module("Qml")
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .qt_module("DBus")
        .qt_module("Network")
        .qrc(root_dir.join("src/resources.qrc"))
        .file("src/qt_backend.rs")
        .file("src/qt_theme.rs")
        .file("src/qt_translator.rs")
        .cpp_files(cpp_sources.iter());

    // SAFETY: We only configure C++17 and include paths for the Qt C++ shims.
    unsafe {
        builder = builder.cc_builder(|cc| {
            cc.std("c++17");
            cc.include(&cpp_dir);
        });
    }

    builder.build();
}
