fn main() {
    let bridges = [
        "src/config.rs",
        "src/i18n.rs",
        "src/meta.rs",
        "src/settings.rs",
        "src/theme.rs",
        "src/util.rs",
    ];
    cxx_build::bridges(bridges)
        .std("c++17")
        .compile("aibox-ffi");

    for bridge in bridges {
        println!("cargo:rerun-if-changed={bridge}");
    }
}
