fn main() {
    let bridges = ["src/config.rs", "src/meta.rs", "src/util.rs"];
    cxx_build::bridges(bridges)
        .std("c++17")
        .compile("aibox-ffi");

    for bridge in bridges {
        println!("cargo:rerun-if-changed={bridge}");
    }
}
