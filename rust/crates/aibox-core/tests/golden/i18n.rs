use crate::common;
use aibox_core::*;
use serde_json::{json, Value};
use std::fs;

fn dispatch(fn_name: &str, _case: &str, args: &Value) -> Value {
    match fn_name {
        "Translator::readLanguage" => {
            let work = "/tmp/aibox-golden-rust-i18n-lang";
            let _ = fs::remove_dir_all(work);
            fs::create_dir_all(work).expect("mkdir");
            let p = format!("{work}/lang");
            if let Some(content) = args["content"].as_str() {
                fs::write(&p, content).expect("write");
            }
            let out = i18n::read_language(&p);
            let _ = fs::remove_dir_all(work);
            json!(out)
        }
        "Translator::pluralCategory" => {
            let lang = args["language"].as_str().expect("language");
            let count = args["count"].as_f64().expect("count") as i64;
            json!(i18n::plural_category(lang, count))
        }
        "Translator::translate" => {
            let lang = args["language"].as_str().expect("language");
            if lang == "pt-BR" {
                let cat = i18n::Catalog::from_json_layers(&[
                    br#"{"%n clips": {"one": "%n clip", "other": "%n clips"}}"#,
                    br#"{"Save": "Guardar", "Open": "Abrir", "Empty": ""}"#,
                    br#"{"Save": "Salvar"}"#,
                ]);
                json!({
                    "Save": cat.lookup("Save", lang, -1).unwrap_or_default(),
                    "Open": cat.lookup("Open", lang, -1).unwrap_or_default(),
                    "EmptyIsNull": cat.lookup("Empty", lang, -1).is_none(),
                    "QuitIsNull": cat.lookup("Quit", lang, -1).is_none(),
                    "Clips1": cat.lookup("%n clips", lang, 1).unwrap_or_default(),
                    "Clips2": cat.lookup("%n clips", lang, 2).unwrap_or_default(),
                })
            } else {
                let cat = i18n::Catalog::from_json_layers(&[
                    br#"{"%n clips": {"one": "%n clip", "other": "%n clips"}}"#,
                    r#"{"%n clips": {"one": "%n клип", "few": "%n клипа", "many": "%n клипов", "other": "%n клипа"}}"#.as_bytes(),
                ]);
                json!({
                    "n21": cat.lookup("%n clips", lang, 21).unwrap_or_default(),
                    "n22": cat.lookup("%n clips", lang, 22).unwrap_or_default(),
                    "n25": cat.lookup("%n clips", lang, 25).unwrap_or_default(),
                    "nNeg": cat.lookup("%n clips", lang, -1).unwrap_or_default(),
                })
            }
        }
        other => panic!("unregistered golden function: {other}"),
    }
}

#[test]
fn replay() {
    common::replay_jsonl("i18n", dispatch);
}
