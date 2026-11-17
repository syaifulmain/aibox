fn main() {
    let bridges = [
        "src/config.rs",
        "src/history.rs",
        "src/i18n.rs",
        "src/meta.rs",
        "src/secrets.rs",
        "src/session.rs",
        "src/settings.rs",
        "src/snapshot.rs",
        "src/theme.rs",
        "src/util.rs",
        "src/workspace_diff.rs",
    ];
    cxx_build::bridges(bridges)
        .std("c++17")
        .compile("aibox-ffi");

    for bridge in bridges {
        println!("cargo:rerun-if-changed={bridge}");
    }
}
