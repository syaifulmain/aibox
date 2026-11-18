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
    "minWidth",
];

/// Explicit allowlist for numeric metric literals:
/// - `components/Style.qml`: defines the design tokens themselves.
/// - `components/SlidersIcon.qml` & `components/BrandMark.qml`: fixed icon geometry.
/// - `dev/ComponentGallery.qml`: standalone developer preview window.
const NUMERIC_LITERAL_ALLOWLIST: &[&str] = &[
    "components/Style.qml",
    "components/SlidersIcon.qml",
    "components/BrandMark.qml",
    "dev/ComponentGallery.qml",
];

#[derive(Default)]
struct FileViolations {
    items: Vec<(usize, String)>,
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
                        if end_ok && num_str != "0" && num_str != "1" {
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

fn find_uncapped_style_radius(line: &str) -> bool {
    let Some(idx) = line.find("radius:") else {
        return false;
    };
    let prev_ok =
        idx == 0 || (!is_ident_char(line.as_bytes()[idx - 1]) && line.as_bytes()[idx - 1] != b'.');
    if !prev_ok {
        return false;
    }
    let rhs = line[idx + "radius:".len()..].trim();
    rhs.contains("Style.radius") && !rhs.contains("theme.radius")
}

fn find_numeric_card_width(line: &str) -> bool {
    let Some(idx) = line.find("cardWidth:") else {
        return false;
    };
    let rhs = line[idx + "cardWidth:".len()..].trim();
    rhs.split(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.')
        .any(|tok| !tok.is_empty() && tok.bytes().all(|b| b.is_ascii_digit()))
}

fn check_footer_primary_limit(content: &str) -> Vec<(usize, String)> {
    let mut violations = Vec::new();
    let mut in_footer = false;
    let mut brace_depth = 0i32;
    let mut footer_start_line = 0usize;
    let mut primary_count = 0usize;

    for (idx, raw_line) in content.lines().enumerate() {
        let line = strip_line_comment(raw_line);
        if !in_footer {
            if let Some(pos) = line.find("DialogFooter") {
                let after = line[pos + "DialogFooter".len()..].trim_start();
                if after.starts_with('{') {
                    in_footer = true;
                    footer_start_line = idx + 1;
                    primary_count = 0;
                    brace_depth = 0;
                }
            }
        }
        if in_footer {
            for b in line.bytes() {
                if b == b'{' {
                    brace_depth += 1;
                } else if b == b'}' {
                    brace_depth -= 1;
                }
            }
            if line.contains("variant:") && line.contains("\"primary\"") {
                primary_count += 1;
            }
            if brace_depth <= 0 {
                if primary_count > 1 {
                    violations.push((
                        footer_start_line,
                        format!("DialogFooter has {primary_count} primary buttons (max 1)"),
                    ));
                }
                in_footer = false;
            }
        }
    }
    violations
}

fn check_modal_cancel_button(content: &str) -> Vec<(usize, String)> {
    let has_modal_header = content.contains("ModalHeader");
    let close_disabled = content.contains("showClose: false");
    if !has_modal_header || close_disabled {
        return Vec::new();
    }
    let mut violations = Vec::new();
    for (idx, raw_line) in content.lines().enumerate() {
        let line = strip_line_comment(raw_line);
        if line.contains("qsTr(\"Cancel\")") {
            violations.push((
                idx + 1,
                "Cancel button is disallowed alongside an active ModalHeader Close button"
                    .to_string(),
            ));
        }
    }
    violations
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
fn enforce_qml_token_and_layout_standards() {
    let qml_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../src/qml");
    let mut files = Vec::new();
    collect_qml_files(&qml_root, &mut files);
    assert!(
        !files.is_empty(),
        "expected QML files under {}",
        qml_root.display()
    );

    let mut violations: BTreeMap<String, FileViolations> = BTreeMap::new();

    for path in &files {
        let rel = path.strip_prefix(&qml_root).unwrap_or(path);
        let rel_str = rel.to_string_lossy().to_string();
        let content = fs::read_to_string(path).expect("read qml file");
        let mut file_v = FileViolations::default();

        let allow_numeric = NUMERIC_LITERAL_ALLOWLIST.contains(&rel_str.as_str());

        for (idx, raw_line) in content.lines().enumerate() {
            let line = strip_line_comment(raw_line);
            for tok in find_deprecated_tokens(line) {
                file_v
                    .items
                    .push((idx + 1, format!("deprecated token `{tok}`")));
            }
            if !allow_numeric {
                for lit in find_numeric_literals(line) {
                    file_v
                        .items
                        .push((idx + 1, format!("numeric literal `{lit}`")));
                }
            }
            if rel_str != "components/Style.qml" && find_uncapped_style_radius(line) {
                file_v.items.push((
                    idx + 1,
                    "Style.radius* must be capped with Math.min(..., theme.radius)".to_string(),
                ));
            }
            if find_numeric_card_width(line) {
                file_v.items.push((
                    idx + 1,
                    "cardWidth must not use numeric literals".to_string(),
                ));
            }
        }

        if rel_str != "dev/ComponentGallery.qml" {
            file_v.items.extend(check_footer_primary_limit(&content));
            file_v.items.extend(check_modal_cancel_button(&content));
        }

        if !file_v.items.is_empty() {
            violations.insert(rel_str, file_v);
        }
    }

    if !violations.is_empty() {
        let mut msg = String::from("QML token/layout lint failed:\n");
        for (file, file_v) in &violations {
            msg.push_str(&format!("  {file}:\n"));
            for (line, reason) in &file_v.items {
                msg.push_str(&format!("    L{line}: {reason}\n"));
            }
        }
        panic!("{msg}");
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
        find_numeric_literals("font.pixelSize: 13; radius: 6; width: 0; height: 1; minWidth: 132"),
        vec!["font.pixelSize: 13", "radius: 6", "minWidth: 132"]
    );
    assert!(find_numeric_literals("spacing: Style.gap; radius: Style.radiusMd").is_empty());

    assert!(find_uncapped_style_radius("radius: Style.radiusMd"));
    assert!(!find_uncapped_style_radius(
        "radius: Math.min(Style.radiusMd, theme.radius)"
    ));
    assert!(!find_uncapped_style_radius("radius: width / 2"));

    assert!(find_numeric_card_width(
        "cardWidth: Math.min(parent.width, 780)"
    ));
    assert!(!find_numeric_card_width(
        "cardWidth: Math.min(targetWidth, parent.width - Style.modalGutter)"
    ));

    let bad_footer = "DialogFooter {\n ActionButton { variant: \"primary\" }\n ActionButton { variant: \"primary\" }\n}";
    assert_eq!(check_footer_primary_limit(bad_footer).len(), 1);
    let ok_footer = "DialogFooter {\n ActionButton { variant: \"default\" }\n ActionButton { variant: \"primary\" }\n}";
    assert!(check_footer_primary_limit(ok_footer).is_empty());

    let bad_modal = "ModalHeader { title: qsTr(\"X\") }\nActionButton { label: qsTr(\"Cancel\") }";
    assert_eq!(check_modal_cancel_button(bad_modal).len(), 1);
}
