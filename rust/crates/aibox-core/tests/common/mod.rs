use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use serde_json::{json, Value};

static ENV_LOCK: Mutex<()> = Mutex::new(());

pub fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
}

/// Serializes golden tests that mutate process-wide environment variables and
/// restores `HOME`, `XDG_CONFIG_HOME`, and `AIBOX_LOCATION_FILE` on drop.
pub struct GoldenEnvGuard {
    _lock: MutexGuard<'static, ()>,
    old_home: Option<OsString>,
    old_xdg: Option<OsString>,
    old_loc: Option<OsString>,
}

impl GoldenEnvGuard {
    pub fn acquire() -> Self {
        let lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let old_home = env::var_os("HOME");
        let old_xdg = env::var_os("XDG_CONFIG_HOME");
        let old_loc = env::var_os("AIBOX_LOCATION_FILE");
        env::set_var("HOME", "/tmp/aibox-golden-home");
        let _ = fs::create_dir_all("/tmp/aibox-golden-home");
        env::remove_var("XDG_CONFIG_HOME");
        env::remove_var("AIBOX_LOCATION_FILE");
        Self {
            _lock: lock,
            old_home,
            old_xdg,
            old_loc,
        }
    }
}

impl Drop for GoldenEnvGuard {
    fn drop(&mut self) {
        if let Some(v) = &self.old_home {
            env::set_var("HOME", v);
        } else {
            env::remove_var("HOME");
        }
        if let Some(v) = &self.old_xdg {
            env::set_var("XDG_CONFIG_HOME", v);
        } else {
            env::remove_var("XDG_CONFIG_HOME");
        }
        if let Some(v) = &self.old_loc {
            env::set_var("AIBOX_LOCATION_FILE", v);
        } else {
            env::remove_var("AIBOX_LOCATION_FILE");
        }
    }
}

pub fn replay_jsonl<F>(module: &str, mut eval: F)
where
    F: FnMut(&str, &str, &Value) -> Value,
{
    let path = golden_dir().join(format!("{module}.jsonl"));
    let contents = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    for (idx, line) in contents.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let row: Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("{}:{}: invalid JSON: {e}", path.display(), idx + 1));
        let fn_name = row["fn"].as_str().expect("missing fn");
        let case = row["case"].as_str().unwrap_or("");
        let mut actual = eval(fn_name, case, &row["args"]);
        if fn_name == "Backend::buildStaleReport" {
            if let Some(ts) = actual.get("generatedAt").and_then(Value::as_str) {
                assert_eq!(ts.len(), 19, "unexpected generatedAt format: {ts}");
            }
            if let Some(obj) = actual.as_object_mut() {
                obj.insert("generatedAt".to_string(), json!("<NOW>"));
            }
        }
        assert_eq!(
            actual,
            row["out"],
            "{}:{}: mismatch for {fn_name} ({case})",
            path.display(),
            idx + 1
        );
    }
}
