use std::path::PathBuf;

use cxx_qt_build::CxxQtBuilder;

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root_dir = manifest_dir
        .join("../../..")
        .canonicalize()
        .expect("repo root");
    let cpp_dir = root_dir.join("src/cpp");
    let cxxbridge_dir = root_dir.join("rust/target/cxxbridge");

    let qobject_headers = [cpp_dir.join("backend.h")];

    let cpp_sources = [
        cpp_dir.join("main.cpp"),
        cpp_dir.join("backend.cpp"),
        cpp_dir.join("backend_config.cpp"),
        cpp_dir.join("backend_backup.cpp"),
        cpp_dir.join("backend_demo.cpp"),
        cpp_dir.join("backend_env.cpp"),
        cpp_dir.join("backend_health.cpp"),
        cpp_dir.join("backend_network.cpp"),
        cpp_dir.join("backend_provision.cpp"),
        cpp_dir.join("backend_secret.cpp"),
        cpp_dir.join("backend_session.cpp"),
        cpp_dir.join("backend_snapshot.cpp"),
        cpp_dir.join("backend_workspace.cpp"),
        cpp_dir.join("cli.cpp"),
        cpp_dir.join("shim/qml_helpers.cpp"),
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
        .file("src/qt_theme.rs")
        .file("src/qt_translator.rs")
        .cpp_files(qobject_headers.iter().chain(cpp_sources.iter()));

    // SAFETY: We only configure C++17 and include paths for the existing C++ QObject facade.
    unsafe {
        builder = builder.cc_builder(|cc| {
            cc.std("c++17");
            cc.include(&cpp_dir);
            cc.include(&cxxbridge_dir);
        });
    }

    builder.build();
}
