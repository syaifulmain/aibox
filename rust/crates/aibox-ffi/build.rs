fn main() {
    let bridges = ["src/cli.rs", "src/meta.rs"];
    cxx_build::bridges(bridges)
        .include("../../../src/cpp")
        .std("c++17")
        .compile("aibox-ffi");

    for bridge in bridges {
        println!("cargo:rerun-if-changed={bridge}");
    }
}
