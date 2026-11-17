use std::os::raw::{c_char, c_int};

use aibox_app as _;

extern "C" {
    fn aibox_test_qml_smoke(err_buf: *mut c_char, err_buf_len: c_int) -> c_int;
}

fn main() {
    let _ = aibox_app::ensure_cxx_qt_linked();
    let mut buf = vec![0u8; 16384];
    let rc = unsafe { aibox_test_qml_smoke(buf.as_mut_ptr() as *mut c_char, buf.len() as c_int) };
    if rc != 0 {
        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        let msg = String::from_utf8_lossy(&buf[..end]);
        eprintln!("QML smoke test failed ({rc} warnings/errors):\n{msg}");
        std::process::exit(1);
    }
    println!("running 1 test");
    println!("test qml_smoke_all_dialogs_and_settings_zero_warnings ... ok");
    println!("test result: ok. 1 passed; 0 failed");
}
