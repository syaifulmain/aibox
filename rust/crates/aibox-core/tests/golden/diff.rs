use crate::common;
use aibox_core::*;
use serde_json::{json, Value};
use std::fs;

fn dispatch(fn_name: &str, _case: &str, args: &Value) -> Value {
    match fn_name {
        "Backend::defaultDiffExcludes" => json!(workspace::diff::default_diff_excludes()),
        "Backend::parseDiffExcludes" => {
            let raw = args["rawText"].as_str().expect("rawText");
            json!(workspace::diff::parse_diff_excludes(raw))
        }
        "Backend::buildRsyncExcludeArgs" => {
            let excludes: Vec<String> = args["excludes"]
                .as_array()
                .expect("excludes")
                .iter()
                .map(|v| v.as_str().expect("string").to_string())
                .collect();
            json!(workspace::diff::build_rsync_exclude_args(&excludes))
        }
        "Backend::buildWorkspaceDiffArgs" => {
            let snap_path = args["snapPath"].as_str().expect("snapPath");
            let ws_path = args["wsPath"].as_str().expect("wsPath");
            let excludes: Vec<String> = args["excludes"]
                .as_array()
                .expect("excludes")
                .iter()
                .map(|v| v.as_str().expect("string").to_string())
                .collect();
            json!(workspace::diff::build_workspace_diff_args(
                snap_path, ws_path, &excludes
            ))
        }
        "Backend::isSafeRelativePath" => {
            let rel = args["relativePath"].as_str().expect("relativePath");
            json!(workspace::diff::is_safe_relative_path(rel))
        }
        "Backend::isLikelyBinaryData" => {
            let hex = args["sampleHex"].as_str().expect("sampleHex");
            let bytes: Vec<u8> = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex byte"))
                .collect();
            json!(workspace::diff::is_likely_binary_data(&bytes))
        }
        "Backend::capDiffLines" => {
            let diff_output = args["diffOutput"].as_str().expect("diffOutput");
            let max_lines = args["maxLines"].as_i64().expect("maxLines") as i32;
            let (text, truncated) = workspace::diff::cap_diff_lines(diff_output, max_lines);
            json!({
                "text": text,
                "truncated": truncated,
            })
        }
        "Backend::parseRsyncItemize" => {
            let output = args["output"].as_str().expect("output");
            let max_entries = args["maxEntries"].as_i64().expect("maxEntries") as i32;
            json!(workspace::diff::parse_rsync_itemize(output, max_entries))
        }
        "Backend::processWorkspaceDiffOutput" => {
            let snap_name = args["snapshotName"].as_str().expect("snapshotName");
            let ws_path = args["wsPath"].as_str().expect("wsPath");
            let stdout = args["rsyncStdout"].as_str().expect("rsyncStdout");
            let _ = fs::remove_dir_all(ws_path);
            fs::create_dir_all(format!("{ws_path}/new dir")).unwrap();
            fs::write(format!("{ws_path}/added file.txt"), "hello world").unwrap();
            std::os::unix::fs::symlink("added file.txt", format!("{ws_path}/added_link")).unwrap();
            let res = workspace::diff::process_workspace_diff_output(snap_name, ws_path, stdout);
            let _ = fs::remove_dir_all(ws_path);
            json!(res)
        }
        other => panic!("unregistered golden function: {other}"),
    }
}

#[test]
fn replay() {
    common::replay_jsonl("diff", dispatch);
}
