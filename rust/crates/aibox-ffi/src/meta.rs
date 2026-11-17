#[cxx::bridge(namespace = "aibox::ffi")]
mod ffi {
    extern "Rust" {
        fn core_version() -> String;
    }
}

fn core_version() -> String {
    aibox_core::core_version().to_string()
}
