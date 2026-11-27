//! Consistent download location inside environments.
//!
//! Downloads are stored in the shared workspace so they appear on the host
//! through the workspace folder, are covered by workspace snapshots and diffs,
//! and stay the same for every application (browsers, Electron apps, `wget`).
//! The `user-dirs.dirs` file is the primary mechanism because desktop
//! applications read it through the XDG base directory specification; the
//! environment variable is a convenience for `aibox enter` and `aibox exec`.

use crate::util::shell::shell_quote;

/// Host-visible download folder inside every environment's workspace.
pub const CONTAINER_DOWNLOAD_DIR: &str = "/home/workspace/download";

/// Renders one `user-dirs.dirs` assignment with a quoted value.
fn download_dir_line(path: &str) -> String {
    // user-dirs.dirs is sourced by shells, so escape characters that would
    // otherwise be expanded or end the quoted value.
    let mut escaped = String::with_capacity(path.len());
    for ch in path.chars() {
        if matches!(ch, '\\' | '"' | '$' | '`') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    format!("XDG_DOWNLOAD_DIR=\"{escaped}\"")
}

/// Computes the `user-dirs.dirs` content for `content` with the download
/// directory pointed at `path`.
///
/// Other `XDG_*_DIR` entries, comments, and blank lines are preserved. An
/// existing `XDG_DOWNLOAD_DIR=` line is replaced in place; duplicate lines are
/// collapsed; otherwise the assignment is appended. The result always ends
/// with a newline, so applying it twice yields the same file.
pub fn upsert_download_dir(content: &str, path: &str) -> String {
    let line = download_dir_line(path);
    let mut lines: Vec<String> = Vec::new();
    let mut replaced = false;
    for existing in content.lines() {
        if existing.trim_start().starts_with("XDG_DOWNLOAD_DIR=") {
            if !replaced {
                lines.push(line.clone());
                replaced = true;
            }
            continue;
        }
        lines.push(existing.to_string());
    }
    if !replaced {
        lines.push(line);
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// Environment exposed to `aibox enter`, `aibox exec`, and GUI Enter.
pub fn download_env() -> Vec<(String, String)> {
    vec![(
        "XDG_DOWNLOAD_DIR".to_string(),
        CONTAINER_DOWNLOAD_DIR.to_string(),
    )]
}

/// Container-side merge mirroring [`upsert_download_dir`].
///
/// `$1` is the canonical assignment, `$2` the full file used when the
/// configuration does not exist yet. Running the merge inside the instance
/// keeps any `XDG_*_DIR` entries that an application or the user added, which a
/// pre-computed file would overwrite. The caller tolerates failures so VMs
/// without an Incus agent still Start/Enter.
const DOWNLOAD_SETUP_SNIPPET: &str = r#"set -e
cfg="${XDG_CONFIG_HOME:-$HOME/.config}"
mkdir -p "$cfg"
file="$cfg/user-dirs.dirs"
tmp="$file.aibox.$$"
trap 'rm -f "$tmp"' EXIT
if [ -f "$file" ]; then
    awk -v line="$1" '
        BEGIN { replaced = 0 }
        /^[[:space:]]*XDG_DOWNLOAD_DIR=/ {
            if (!replaced) { print line; replaced = 1 }
            next
        }
        { print }
        END { if (!replaced) print line }
    ' "$file" > "$tmp"
else
    printf '%s' "$2" > "$tmp"
fi
mv "$tmp" "$file"
trap - EXIT"#;

/// Host-side snippet that creates the shared download folder and points
/// `XDG_DOWNLOAD_DIR` at it. It runs on every Start/Enter and after Create, so
/// existing environments are fixed up on their next session and the operation
/// is idempotent.
pub fn download_dir_script(name: &str) -> String {
    let q_name = shell_quote(name);
    let q_dir = shell_quote(CONTAINER_DOWNLOAD_DIR);
    let q_line = shell_quote(&download_dir_line(CONTAINER_DOWNLOAD_DIR));
    let q_fresh = shell_quote(&upsert_download_dir("", CONTAINER_DOWNLOAD_DIR));
    let combined = format!("mkdir -p {q_dir} >/dev/null 2>&1 || true\n{DOWNLOAD_SETUP_SNIPPET}");
    format!(
        "incus exec {q_name} -- sh -c {} aibox {q_line} {q_fresh} >/dev/null 2>&1 || true",
        shell_quote(&combined)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    fn temp_dir(label: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!("aibox-download-{label}-{nanos}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn write_stub_incus(dir: &Path) -> PathBuf {
        fs::create_dir_all(dir).expect("create stub dir");
        let path = dir.join("incus");
        fs::write(
            &path,
            "#!/bin/sh\nif [ \"$1\" = \"exec\" ]; then\n    shift 3\n    exec \"$@\"\nfi\nexit 1\n",
        )
        .expect("write stub");
        let mut perms = fs::metadata(&path).expect("stub metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("chmod stub");
        path
    }

    fn run_on_fake_container(script: &str, home: &Path, stub_dir: &Path) {
        let path_env = format!(
            "{}:{}",
            stub_dir.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let output = Command::new("/bin/bash")
            .arg("-c")
            .arg(script)
            .env("HOME", home)
            .env("PATH", path_env)
            .env_remove("XDG_CONFIG_HOME")
            .output()
            .expect("run generated script");
        assert!(
            output.status.success(),
            "script failed:\nstdout: {}\nstderr: {}\n{script}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn replaces_an_existing_download_line_in_place() {
        let existing = "# written by hand\nXDG_DESKTOP_DIR=\"$HOME/Desktop\"\nXDG_DOWNLOAD_DIR=\"$HOME/Downloads\"\nXDG_MUSIC_DIR=\"$HOME/Music\"\n";
        let updated = upsert_download_dir(existing, CONTAINER_DOWNLOAD_DIR);
        assert_eq!(
            updated,
            "# written by hand\nXDG_DESKTOP_DIR=\"$HOME/Desktop\"\nXDG_DOWNLOAD_DIR=\"/home/workspace/download\"\nXDG_MUSIC_DIR=\"$HOME/Music\"\n"
        );
        assert_eq!(updated.matches("XDG_DOWNLOAD_DIR=").count(), 1);
    }

    #[test]
    fn appends_the_line_and_preserves_other_entries() {
        let existing = "XDG_DOCUMENTS_DIR=\"$HOME/Documents\"\n";
        let updated = upsert_download_dir(existing, CONTAINER_DOWNLOAD_DIR);
        assert_eq!(
            updated,
            "XDG_DOCUMENTS_DIR=\"$HOME/Documents\"\nXDG_DOWNLOAD_DIR=\"/home/workspace/download\"\n"
        );
    }

    #[test]
    fn empty_content_creates_only_the_download_assignment() {
        assert_eq!(
            upsert_download_dir("", CONTAINER_DOWNLOAD_DIR),
            "XDG_DOWNLOAD_DIR=\"/home/workspace/download\"\n"
        );
        assert_eq!(
            upsert_download_dir("\n\n", CONTAINER_DOWNLOAD_DIR),
            "\n\nXDG_DOWNLOAD_DIR=\"/home/workspace/download\"\n"
        );
    }

    #[test]
    fn duplicate_download_lines_collapse_and_upsert_is_idempotent() {
        let existing = "XDG_DOWNLOAD_DIR=\"$HOME/a\"\nXDG_VIDEOS_DIR=\"$HOME/Videos\"\nXDG_DOWNLOAD_DIR=\"$HOME/b\"\n";
        let once = upsert_download_dir(existing, CONTAINER_DOWNLOAD_DIR);
        assert_eq!(once.matches("XDG_DOWNLOAD_DIR=").count(), 1);
        assert!(once.contains("XDG_VIDEOS_DIR=\"$HOME/Videos\""));
        assert_eq!(
            upsert_download_dir(&once, CONTAINER_DOWNLOAD_DIR),
            once,
            "second application must not change the file"
        );
    }

    #[test]
    fn quoted_values_escape_shell_metacharacters() {
        let updated = upsert_download_dir("", "/tmp/a $b `c` \\d \"e\"");
        assert_eq!(
            updated,
            "XDG_DOWNLOAD_DIR=\"/tmp/a \\$b \\`c\\` \\\\d \\\"e\\\"\"\n"
        );
    }

    #[test]
    fn environment_exposes_the_shared_download_path() {
        assert_eq!(
            download_env(),
            vec![(
                "XDG_DOWNLOAD_DIR".to_string(),
                CONTAINER_DOWNLOAD_DIR.to_string()
            )]
        );
    }

    #[test]
    fn script_creates_the_folder_and_quotes_the_instance_name() {
        let script = download_dir_script("it's a box; rm -rf /");
        assert!(script.contains("'it'\\''s a box; rm -rf /'"));
        assert!(!script.contains("; rm -rf /\n"));
        assert!(script.contains("mkdir -p ") && script.contains("/home/workspace/download"));
        assert!(script.contains("$HOME/.config"));
    }

    #[test]
    fn script_merges_inside_the_instance_without_losing_other_entries() {
        let base = temp_dir("merge");
        let stub_dir = base.join("stubs");
        write_stub_incus(&stub_dir);
        let home = base.join("home");
        fs::create_dir_all(home.join(".config")).expect("create config dir");
        let config = home.join(".config/user-dirs.dirs");
        let existing = "XDG_DESKTOP_DIR=\"$HOME/Desktop\"\nXDG_DOWNLOAD_DIR=\"$HOME/Downloads\"\nXDG_MUSIC_DIR=\"$HOME/Music\"\n";
        fs::write(&config, existing).expect("seed user-dirs");

        let script = download_dir_script("box");
        run_on_fake_container(&script, &home, &stub_dir);
        let written = fs::read_to_string(&config).expect("read user-dirs");
        assert_eq!(
            written,
            upsert_download_dir(existing, CONTAINER_DOWNLOAD_DIR)
        );

        run_on_fake_container(&script, &home, &stub_dir);
        assert_eq!(
            fs::read_to_string(&config).expect("read user-dirs"),
            written,
            "second run must not change the file"
        );

        let fresh_home = base.join("fresh-home");
        fs::create_dir_all(&fresh_home).expect("create fresh home");
        run_on_fake_container(&script, &fresh_home, &stub_dir);
        assert_eq!(
            fs::read_to_string(fresh_home.join(".config/user-dirs.dirs"))
                .expect("read fresh user-dirs"),
            upsert_download_dir("", CONTAINER_DOWNLOAD_DIR)
        );

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn generated_scripts_pass_bash_syntax_check() {
        for name in ["box", "it's a box; rm -rf /"] {
            let script = download_dir_script(name);
            let status = Command::new("bash")
                .arg("-n")
                .arg("-c")
                .arg(&script)
                .status()
                .expect("run bash -n");
            assert!(status.success(), "bash -n failed:\n{script}");
        }
    }
}
