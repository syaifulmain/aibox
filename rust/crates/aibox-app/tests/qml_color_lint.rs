use std::fs;
use std::path::{Path, PathBuf};

const CSS_COLORS: &[&str] = &[
    "aliceblue",
    "antiquewhite",
    "aqua",
    "aquamarine",
    "azure",
    "beige",
    "bisque",
    "black",
    "blanchedalmond",
    "blue",
    "blueviolet",
    "brown",
    "burlywood",
    "cadetblue",
    "chartreuse",
    "chocolate",
    "coral",
    "cornflowerblue",
    "cornsilk",
    "crimson",
    "cyan",
    "darkblue",
    "darkcyan",
    "darkgoldenrod",
    "darkgray",
    "darkgreen",
    "darkgrey",
    "darkkhaki",
    "darkmagenta",
    "darkolivegreen",
    "darkorange",
    "darkorchid",
    "darkred",
    "darksalmon",
    "darkseagreen",
    "darkslateblue",
    "darkslategray",
    "darkslategrey",
    "darkturquoise",
    "darkviolet",
    "deeppink",
    "deepskyblue",
    "dimgray",
    "dimgrey",
    "dodgerblue",
    "firebrick",
    "floralwhite",
    "forestgreen",
    "fuchsia",
    "gainsboro",
    "ghostwhite",
    "gold",
    "goldenrod",
    "gray",
    "green",
    "greenyellow",
    "grey",
    "honeydew",
    "hotpink",
    "indianred",
    "indigo",
    "ivory",
    "khaki",
    "lavender",
    "lavenderblush",
    "lawngreen",
    "lemonchiffon",
    "lightblue",
    "lightcoral",
    "lightcyan",
    "lightgoldenrodyellow",
    "lightgray",
    "lightgreen",
    "lightgrey",
    "lightpink",
    "lightsalmon",
    "lightseagreen",
    "lightskyblue",
    "lightslategray",
    "lightslategrey",
    "lightsteelblue",
    "lightyellow",
    "lime",
    "limegreen",
    "linen",
    "magenta",
    "maroon",
    "mediumaquamarine",
    "mediumblue",
    "mediumorchid",
    "mediumpurple",
    "mediumseagreen",
    "mediumslateblue",
    "mediumspringgreen",
    "mediumturquoise",
    "mediumvioletred",
    "midnightblue",
    "mintcream",
    "mistyrose",
    "moccasin",
    "navajowhite",
    "navy",
    "oldlace",
    "olive",
    "olivedrab",
    "orange",
    "orangered",
    "orchid",
    "palegoldenrod",
    "palegreen",
    "paleturquoise",
    "palevioletred",
    "papayawhip",
    "peachpuff",
    "peru",
    "pink",
    "plum",
    "powderblue",
    "purple",
    "rebeccapurple",
    "red",
    "rosybrown",
    "royalblue",
    "saddlebrown",
    "salmon",
    "sandybrown",
    "seagreen",
    "seashell",
    "sienna",
    "silver",
    "skyblue",
    "slateblue",
    "slategray",
    "slategrey",
    "snow",
    "springgreen",
    "steelblue",
    "tan",
    "teal",
    "thistle",
    "tomato",
    "turquoise",
    "violet",
    "wheat",
    "white",
    "whitesmoke",
    "yellow",
    "yellowgreen",
];

const LEGACY_THEME_TOKENS: &[&str] = &[
    "muted",
    "foreground",
    "red",
    "green",
    "yellow",
    "lighterBackground",
    "darkBackground",
    "background",
    "accentForeground",
];

const LEGACY_STYLE_PREFIXES: &[&str] =
    &["surface", "border", "danger", "success", "warning", "scrim"];

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn check_quoted_literals(line: &str, reasons: &mut Vec<String>) {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let quote = bytes[i];
        if quote != b'"' && quote != b'\'' {
            i += 1;
            continue;
        }
        let start = i + 1;
        let mut end = start;
        while end < bytes.len() {
            if bytes[end] == b'\\' {
                end += 2;
                continue;
            }
            if bytes[end] == quote {
                break;
            }
            end += 1;
        }
        if end < bytes.len() {
            let inner = &line[start..end];
            if let Some(hex) = inner.strip_prefix('#') {
                if (3..=8).contains(&hex.len()) && hex.bytes().all(|c| c.is_ascii_hexdigit()) {
                    reasons.push(format!("hardcoded hex color literal {quote}{inner}{quote}"));
                }
            } else {
                let lower = inner.to_ascii_lowercase();
                if lower != "transparent" && CSS_COLORS.binary_search(&lower.as_str()).is_ok() {
                    reasons.push(format!("hardcoded CSS color literal {quote}{inner}{quote}"));
                }
            }
            i = end + 1;
        } else {
            break;
        }
    }
}

fn check_prefix_tokens(
    line: &str,
    prefix: &str,
    exact_only: bool,
    list: &[&str],
    reasons: &mut Vec<String>,
) {
    let mut search_from = 0;
    while let Some(rel_idx) = line[search_from..].find(prefix) {
        let abs_idx = search_from + rel_idx;
        let prev_ok = abs_idx == 0 || !is_ident_char(line.as_bytes()[abs_idx - 1]);
        let after = &line[abs_idx + prefix.len()..];
        if prev_ok {
            let ident_len = after.bytes().take_while(|b| is_ident_char(*b)).count();
            let ident = &after[..ident_len];
            let matched = if exact_only {
                list.contains(&ident)
            } else {
                list.iter().any(|p| ident.starts_with(p))
            };
            if matched {
                reasons.push(format!("legacy token `{prefix}{ident}`"));
            }
        }
        search_from = abs_idx + prefix.len();
    }
}

fn lint_line(line: &str) -> Vec<String> {
    let mut reasons = Vec::new();
    check_quoted_literals(line, &mut reasons);

    for fn_call in ["Qt.rgba(", "Qt.lighter(", "Qt.darker("] {
        if line.contains(fn_call) {
            reasons.push(format!("disallowed QML color derivation `{fn_call}`"));
        }
    }

    check_prefix_tokens(line, "theme.", true, LEGACY_THEME_TOKENS, &mut reasons);
    check_prefix_tokens(line, "Style.", false, LEGACY_STYLE_PREFIXES, &mut reasons);
    reasons
}

fn collect_qml_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .expect("read qml dir")
        .map(|e| e.expect("dir entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_qml_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("qml") {
            out.push(path);
        }
    }
}

#[test]
fn qml_files_contain_no_hardcoded_colors_or_legacy_tokens() {
    let qml_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../src/qml");
    let mut files = Vec::new();
    collect_qml_files(&qml_root, &mut files);
    assert!(
        !files.is_empty(),
        "expected QML files under {}",
        qml_root.display()
    );

    let mut violations = Vec::new();
    for path in &files {
        let rel = path.strip_prefix(&qml_root).unwrap_or(path);
        let content = fs::read_to_string(path).expect("read qml file");
        for (idx, line) in content.lines().enumerate() {
            for reason in lint_line(line) {
                violations.push(format!(
                    "{}:{}: {reason}: {}",
                    rel.display(),
                    idx + 1,
                    line.trim()
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "QML color lint failed with {} violation(s):\n{}",
        violations.len(),
        violations.join("\n")
    );
}

#[test]
fn lint_rules_catch_all_disallowed_patterns_and_allow_valid_tokens() {
    assert!(!lint_line(r##"color: "#12131c""##).is_empty());
    assert!(!lint_line(r##"color: "#fff""##).is_empty());
    assert!(!lint_line(r##"color: "#8012131c""##).is_empty());
    assert!(!lint_line(r#"color: "black""#).is_empty());
    assert!(!lint_line(r#"color: "white""#).is_empty());
    assert!(!lint_line(r#"color: "red""#).is_empty());
    assert!(!lint_line(r#"color: Qt.rgba(1, 0, 0, 0.5)"#).is_empty());
    assert!(!lint_line(r#"color: Qt.lighter(theme.accent, 1.2)"#).is_empty());
    assert!(!lint_line(r#"color: Qt.darker(theme.surfaceCard, 1.1)"#).is_empty());
    assert!(!lint_line(r#"color: theme.muted"#).is_empty());
    assert!(!lint_line(r#"color: theme.foreground"#).is_empty());
    assert!(!lint_line(r#"color: theme.darkBackground"#).is_empty());
    assert!(!lint_line(r#"color: theme.accentForeground"#).is_empty());
    assert!(!lint_line(r#"color: Style.surfaceCard"#).is_empty());
    assert!(!lint_line(r#"color: Style.borderSubtle"#).is_empty());
    assert!(!lint_line(r#"color: Style.dangerSurface"#).is_empty());

    assert!(lint_line(r#"color: "transparent""#).is_empty());
    assert!(lint_line(r#"color: theme.surfaceCard"#).is_empty());
    assert!(lint_line(r#"color: theme.textMuted"#).is_empty());
    assert!(lint_line(r#"color: theme.accentText"#).is_empty());
    assert!(lint_line(r#"radius: Style.radius"#).is_empty());
}
