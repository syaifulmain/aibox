fn main() {
    let bridges = [
        "src/backup.rs",
        "src/config.rs",
        "src/health.rs",
        "src/history.rs",
        "src/i18n.rs",
        "src/incus.rs",
        "src/meta.rs",
        "src/network.rs",
        "src/provision.rs",
        "src/secrets.rs",
        "src/session.rs",
        "src/settings.rs",
        "src/snapshot.rs",
        "src/task.rs",
        "src/theme.rs",
        "src/util.rs",
        "src/workspace_diff.rs",
        "src/workspace_workflow.rs",
    ];
    cxx_build::bridges(bridges)
        .include("../../../src/cpp")
        .std("c++17")
        .compile("aibox-ffi");

    for bridge in bridges {
        println!("cargo:rerun-if-changed={bridge}");
    }
}
