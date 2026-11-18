use crate::common;
use aibox_core::*;
use serde_json::{json, Value};

fn dispatch(fn_name: &str, _case: &str, args: &Value) -> Value {
    match fn_name {
        "Theme::parseColors" => {
            let toml = args["toml"].as_str().expect("toml");
            let c = theme::parse_colors(toml);
            json!({
                "accent": c.accent,
                "accentForeground": theme::accent_foreground(&c.accent),
                "background": c.background,
                "darkBackground": c.dark_background,
                "lighterBackground": c.lighter_background,
                "foreground": c.foreground,
                "muted": c.muted,
                "green": c.green,
                "red": c.red,
                "yellow": c.yellow,
            })
        }
        "Theme::accentForeground" => {
            let hex = args["accent"].as_str().expect("accent");
            json!(theme::accent_foreground(hex))
        }
        other => panic!("unregistered golden function: {other}"),
    }
}

#[test]
fn replay() {
    common::replay_jsonl("theme", dispatch);
}
