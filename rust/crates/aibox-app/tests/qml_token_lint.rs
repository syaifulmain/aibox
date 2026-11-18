use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const DEPRECATED_STYLE_TOKENS: &[&str] = &[
    "font8",
    "font9",
    "font10",
    "font11",
    "font12",
    "font13",
    "font14",
    "font15",
    "font16",
    "font18",
    "space1",
    "space2",
    "space3",
    "space4",
    "space5",
    "space6",
    "space7",
    "space8",
    "space10",
    "space12",
    "space14",
    "space16",
    "space18",
    "space20",
    "space22",
    "space24",
    "space30",
    "cardPadding",
    "cardPaddingSm",
    "cardPaddingXs",
    "radius2",
    "radius3",
    "radius4",
    "radius5",
    "radius6",
    "radius8",
    "controlHeightSm",
    "controlHeightXs",
    "controlHeightXxs",
    "badgeHeightMd",
    "badgeHeightLg",
    "menuRowHeight",
    "modalGutterWide",
    "modalEdge",
    "modalEdgeTight",
    "modalPaddingTight",
];

const METRIC_PROPERTIES: &[&str] = &[
    "font.pixelSize",
    "spacing",
    "columnSpacing",
    "rowSpacing",
    "radius",
    "width",
    "height",
    "implicitWidth",
    "implicitHeight",
    "margins",
    "topMargin",
    "bottomMargin",
    "leftMargin",
    "rightMargin",
    "padding",
    "topPadding",
    "bottomPadding",
    "leftPadding",
    "rightPadding",
];

#[derive(Default)]
struct FileReport {
    deprecated_tokens: Vec<(usize, String)>,
    numeric_literals: Vec<(usize, String)>,
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn strip_line_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_quote: Option<u8> = None;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = in_quote {
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == q {
                in_quote = None;
            }
            i += 1;
            continue;
        }
        if b == b'"' || b == b'\'' {
            in_quote = Some(b);
            i += 1;
            continue;
        }
        if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            return &line[..i];
        }
        i += 1;
    }
    line
}

fn find_deprecated_tokens(line: &str) -> Vec<String> {
    let mut found = Vec::new();
    let prefix = "Style.";
    let mut search_from = 0;
    while let Some(rel_idx) = line[search_from..].find(prefix) {
        let abs_idx = search_from + rel_idx;
        let prev_ok = abs_idx == 0 || !is_ident_char(line.as_bytes()[abs_idx - 1]);
        if prev_ok {
            let after = &line[abs_idx + prefix.len()..];
            let ident_len = after.bytes().take_while(|b| is_ident_char(*b)).count();
            let ident = &after[..ident_len];
            if DEPRECATED_STYLE_TOKENS.contains(&ident) {
                found.push(format!("{prefix}{ident}"));
            }
        }
        search_from = abs_idx + prefix.len();
    }
    found
}

fn find_numeric_literals(line: &str) -> Vec<String> {
    let mut found = Vec::new();
    for prop in METRIC_PROPERTIES {
        let mut search_from = 0;
        while let Some(rel_idx) = line[search_from..].find(prop) {
            let abs_idx = search_from + rel_idx;
            let prev_ok = abs_idx == 0
                || (!is_ident_char(line.as_bytes()[abs_idx - 1])
                    && line.as_bytes()[abs_idx - 1] != b'.');
            let after_prop = &line[abs_idx + prop.len()..];
            if prev_ok {
                let trimmed = after_prop.trim_start();
                if let Some(after_colon) = trimmed.strip_prefix(':') {
                    let rhs = after_colon.trim_start();
                    let num_len = rhs.bytes().take_while(|b| b.is_ascii_digit()).count();
                    if num_len > 0 {
                        let next_byte = rhs.as_bytes().get(num_len).copied();
                        let end_ok = next_byte.is_none_or(|b| {
                            b.is_ascii_whitespace() || b == b';' || b == b'}' || b == b','
                        });
                        let num_str = &rhs[..num_len];
                        if end_ok && num_str != "0" {
                            found.push(format!("{prop}: {num_str}"));
                        }
                    }
                }
            }
            search_from = abs_idx + prop.len();
        }
    }
    found
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
fn report_qml_token_usage_and_numeric_literals() {
    let qml_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../src/qml");
    let mut files = Vec::new();
    collect_qml_files(&qml_root, &mut files);
    assert!(
        !files.is_empty(),
        "expected QML files under {}",
        qml_root.display()
    );

    let mut reports: BTreeMap<String, FileReport> = BTreeMap::new();
    let mut total_deprecated = 0usize;
    let mut total_literals = 0usize;

    for path in &files {
        let rel = path.strip_prefix(&qml_root).unwrap_or(path);
        let rel_str = rel.to_string_lossy().to_string();
        if rel_str == "components/Style.qml" {
            continue;
        }
        let content = fs::read_to_string(path).expect("read qml file");
        let mut report = FileReport::default();

        for (idx, raw_line) in content.lines().enumerate() {
            let line = strip_line_comment(raw_line);
            for tok in find_deprecated_tokens(line) {
                total_deprecated += 1;
                report.deprecated_tokens.push((idx + 1, tok));
            }
            for lit in find_numeric_literals(line) {
                total_literals += 1;
                report.numeric_literals.push((idx + 1, lit));
            }
        }

        if !report.deprecated_tokens.is_empty() || !report.numeric_literals.is_empty() {
            reports.insert(rel_str, report);
        }
    }

    eprintln!(
        "QML token lint (report mode): {} file(s) with findings, {} deprecated token(s), {} numeric literal(s)",
        reports.len(),
        total_deprecated,
        total_literals
    );
    for (file, report) in &reports {
        eprintln!(
            "  {file}: {} deprecated, {} numeric literal(s)",
            report.deprecated_tokens.len(),
            report.numeric_literals.len()
        );
        for (line, tok) in &report.deprecated_tokens {
            eprintln!("    L{line}: deprecated `{tok}`");
        }
        for (line, lit) in &report.numeric_literals {
            eprintln!("    L{line}: numeric literal `{lit}`");
        }
    }
}

#[test]
fn token_lint_detectors_match_expected_patterns() {
    assert_eq!(
        find_deprecated_tokens("spacing: Style.space10; font.pixelSize: Style.font13"),
        vec!["Style.space10", "Style.font13"]
    );
    assert!(
        find_deprecated_tokens("spacing: Style.gap; font.pixelSize: Style.fontBody").is_empty()
    );

    assert_eq!(
        find_numeric_literals("font.pixelSize: 13; radius: 6; width: 0"),
        vec!["font.pixelSize: 13", "radius: 6"]
    );
    assert!(find_numeric_literals("spacing: Style.gap; radius: Style.radiusMd").is_empty());
}
