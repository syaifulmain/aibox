#[cxx::bridge(namespace = "aibox::ffi")]
mod ffi {
    extern "Rust" {
        fn format_bytes(bytes: i64) -> String;
        fn format_byte_rate(bytes_per_second: i64) -> String;
        fn format_byte_total(bytes: i64) -> String;
        fn parse_size_bytes(size_text: &str) -> i64;
        fn validated_size_options(values: Vec<String>) -> Vec<String>;
        fn validated_cpu_options_json(json_array: &str) -> Vec<i32>;
        fn shell_quote(value: &str) -> String;
        fn shell_array_words(words: &str) -> String;
        fn parse_csv(text: &str) -> Vec<String>;
        fn sanitize_name(raw_name: &str) -> String;
        fn truncate_log_tail(data: &[u8], max_bytes: i64) -> Vec<u8>;
    }
}

fn format_bytes(bytes: i64) -> String {
    aibox_core::util::format::format_bytes(bytes)
}

fn format_byte_rate(bytes_per_second: i64) -> String {
    aibox_core::util::format::format_byte_rate(bytes_per_second)
}

fn format_byte_total(bytes: i64) -> String {
    aibox_core::util::format::format_byte_total(bytes)
}

fn parse_size_bytes(size_text: &str) -> i64 {
    aibox_core::util::size::parse_size_bytes(size_text)
}

fn validated_size_options(values: Vec<String>) -> Vec<String> {
    aibox_core::util::size::validated_size_options(&values)
}

fn validated_cpu_options_json(json_array: &str) -> Vec<i32> {
    aibox_core::util::size::validated_cpu_options_json(json_array)
}

fn shell_quote(value: &str) -> String {
    aibox_core::util::shell::shell_quote(value)
}

fn shell_array_words(words: &str) -> String {
    aibox_core::util::shell::shell_array_words(words)
}

fn parse_csv(text: &str) -> Vec<String> {
    aibox_core::util::csv::parse_csv(text)
}

fn sanitize_name(raw_name: &str) -> String {
    aibox_core::util::names::sanitize_name(raw_name)
}

fn truncate_log_tail(data: &[u8], max_bytes: i64) -> Vec<u8> {
    aibox_core::util::bytes::truncate_log_tail(data, max_bytes)
}
