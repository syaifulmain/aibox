//! Optional non-root default user provisioning and exec-flag helpers.

use std::sync::OnceLock;

use regex::Regex;

use crate::provision::ProvisionError;
use crate::util::shell::shell_quote;

/// Non-root login user persisted on an environment via `user.aibox.*` config keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginUser {
    pub name: String,
    pub uid: u32,
    pub gid: u32,
}

impl LoginUser {
    pub fn home(&self) -> String {
        format!("/home/{}", self.name)
    }
}

/// Request to create (or reuse) a non-root default user during environment creation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewUserSpec {
    pub name: String,
    pub sudo: bool,
    pub preferred_uid: u32,
}

const RESERVED_USERNAMES: &[&str] = &["root", "workspace", "nobody", "daemon", "bin", "sys"];

/// Validates a Linux username for the non-root default user.
pub fn validate_login_username(raw: &str) -> Result<String, ProvisionError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"^[a-z_][a-z0-9_-]{0,31}$").expect("valid login username regex")
    });
    let trimmed = raw.trim();
    if !re.is_match(trimmed) || RESERVED_USERNAMES.contains(&trimmed) {
        return Err(ProvisionError::InvalidLoginUser);
    }
    Ok(trimmed.to_string())
}

/// Derives a valid default username from the host `$USER` value.
pub fn suggested_login_user(host_user: &str) -> String {
    let trimmed = host_user.trim();
    if trimmed.is_empty() {
        return "dev".to_string();
    }
    let mut sanitized = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_lowercase() || lower.is_ascii_digit() || lower == '-' || lower == '_' {
            sanitized.push(lower);
        } else {
            sanitized.push('-');
        }
    }
    if sanitized
        .chars()
        .next()
        .is_none_or(|c| !(c.is_ascii_lowercase() || c == '_'))
    {
        sanitized.insert(0, '_');
    }
    let truncated: String = sanitized.chars().take(32).collect();
    validate_login_username(&truncated).unwrap_or_else(|_| "dev".to_string())
}

/// Builds `incus exec` arguments that run the session as `user`.
pub fn exec_user_args(user: &LoginUser) -> Vec<String> {
    vec![
        "--user".to_string(),
        user.uid.to_string(),
        "--group".to_string(),
        user.gid.to_string(),
        "--env".to_string(),
        format!("HOME={}", user.home()),
        "--env".to_string(),
        format!("USER={}", user.name),
        "--env".to_string(),
        format!("LOGNAME={}", user.name),
    ]
}

/// Builds shell-quoted `incus exec` user flags with a leading space.
pub fn exec_user_flags_shell(user: &LoginUser) -> String {
    let uid_s = user.uid.to_string();
    let gid_s = user.gid.to_string();
    let home_env = format!("HOME={}", user.home());
    let user_env = format!("USER={}", user.name);
    let logname_env = format!("LOGNAME={}", user.name);
    format!(
        " --user {} --group {} --env {} --env {} --env {}",
        shell_quote(&uid_s),
        shell_quote(&gid_s),
        shell_quote(&home_env),
        shell_quote(&user_env),
        shell_quote(&logname_env),
    )
}

/// Generates the bash snippet appended to the create script to provision `spec`.
pub fn create_user_snippet(spec: &NewUserSpec) -> String {
    let q_name = shell_quote(&spec.name);
    let want_sudo = if spec.sudo { "1" } else { "0" };
    let inner = r#"set -eu; u="$1"; pref="$2"; want_sudo="$3"
uid_taken() {
    if command -v getent >/dev/null 2>&1; then
        getent passwd "$1" >/dev/null 2>&1
    else
        grep -q "^[^:]*:[^:]*:$1:" /etc/passwd
    fi
}
shell=/bin/sh; [ -x /bin/bash ] && shell=/bin/bash
if ! id "$u" >/dev/null 2>&1; then
    if command -v useradd >/dev/null 2>&1; then
        if uid_taken "$pref"; then useradd -m -U -s "$shell" "$u"; else useradd -m -U -u "$pref" -s "$shell" "$u"; fi
    elif command -v adduser >/dev/null 2>&1; then
        if uid_taken "$pref"; then adduser -D -s "$shell" "$u"; else adduser -D -u "$pref" -s "$shell" "$u"; fi
    else
        echo "No useradd/adduser in this image." >&2
        exit 1
    fi
fi
if [ "$want_sudo" = 1 ]; then
    if ! command -v sudo >/dev/null 2>&1; then
        {
            if command -v apt-get >/dev/null 2>&1; then
                DEBIAN_FRONTEND=noninteractive apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq sudo
            elif command -v pacman >/dev/null 2>&1; then
                pacman -Sy --noconfirm --needed sudo
            elif command -v dnf >/dev/null 2>&1; then
                dnf -y -q install sudo
            elif command -v apk >/dev/null 2>&1; then
                apk add --no-cache sudo
            else
                false
            fi
        } >/dev/null 2>&1 || echo "Warning: could not install sudo; '$u' has no sudo access." >&2
    fi
    mkdir -p /etc/sudoers.d
    printf '%s ALL=(ALL) NOPASSWD:ALL\n' "$u" > /etc/sudoers.d/90-aibox-user
    chmod 0440 /etc/sudoers.d/90-aibox-user
fi"#;
    let q_inner = shell_quote(inner);
    format!(
        "echo \"@@STAGE:94:Creating user '{name}'...\"\n\
         for _ in $(seq 1 60); do incus exec \"$NAME\" -- true >/dev/null 2>&1 && break; sleep 1; done\n\
         incus exec \"$NAME\" -- sh -c {q_inner} aibox-user {q_name} {uid} {want_sudo}\n\
         AIBOX_UID=$(incus exec \"$NAME\" -- id -u {q_name})\n\
         AIBOX_GID=$(incus exec \"$NAME\" -- id -g {q_name})\n\
         incus config set \"$NAME\" user.aibox.user={q_name} user.aibox.uid=\"$AIBOX_UID\" user.aibox.gid=\"$AIBOX_GID\"\n",
        name = spec.name,
        uid = spec.preferred_uid,
    )
}

/// Ensures `XDG_RUNTIME_DIR` inside the container is owned by `user` so desktop
/// and audio sockets created under `/run/user/<uid>` remain accessible.
pub fn runtime_dir_owner_script(
    env_name: &str,
    user: &LoginUser,
    session_env: &[(String, String)],
) -> String {
    let Some((_, runtime_dir)) = session_env.iter().find(|(k, _)| k == "XDG_RUNTIME_DIR") else {
        return String::new();
    };
    if runtime_dir.trim().is_empty() {
        return String::new();
    }
    let q_name = shell_quote(env_name);
    let q_dir = shell_quote(runtime_dir);
    format!(
        "incus exec {q_name} -- sh -c 'mkdir -p \"$1\" && chown \"$2:$3\" \"$1\" && chmod 0700 \"$1\"' aibox-rt {q_dir} {} {} >/dev/null 2>&1 || true",
        user.uid, user.gid
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_login_username_accepts_and_rejects_correctly() {
        assert_eq!(validate_login_username("dev").unwrap(), "dev");
        assert_eq!(validate_login_username("  saep_1-a ").unwrap(), "saep_1-a");
        assert_eq!(validate_login_username("_bot").unwrap(), "_bot");

        for bad in [
            "",
            "root",
            "workspace",
            "nobody",
            "daemon",
            "bin",
            "sys",
            "Root",
            "1user",
            "-user",
            "user.name",
            "a".repeat(33).as_str(),
        ] {
            assert_eq!(
                validate_login_username(bad).unwrap_err(),
                ProvisionError::InvalidLoginUser,
                "expected rejection for {bad:?}"
            );
        }
    }

    #[test]
    fn suggested_login_user_sanitizes_host_user() {
        assert_eq!(suggested_login_user("Saep.User"), "saep-user");
        assert_eq!(suggested_login_user("root"), "dev");
        assert_eq!(suggested_login_user("workspace"), "dev");
        assert_eq!(suggested_login_user("123alice"), "_123alice");
        assert_eq!(suggested_login_user(""), "dev");
    }

    #[test]
    fn exec_user_args_and_shell_flags_match_expected_format() {
        let user = LoginUser {
            name: "dev".to_string(),
            uid: 1000,
            gid: 1000,
        };
        assert_eq!(
            exec_user_args(&user),
            vec![
                "--user",
                "1000",
                "--group",
                "1000",
                "--env",
                "HOME=/home/dev",
                "--env",
                "USER=dev",
                "--env",
                "LOGNAME=dev",
            ]
        );
        let flags = exec_user_flags_shell(&user);
        assert!(flags.starts_with(" --user "));
        assert!(flags.contains("'HOME=/home/dev'"));
        assert!(flags.contains("'USER=dev'"));
        assert!(flags.contains("'LOGNAME=dev'"));
    }

    #[test]
    fn create_user_snippet_includes_useradd_uid_and_conditional_sudo() {
        let with_sudo = create_user_snippet(&NewUserSpec {
            name: "dev".to_string(),
            sudo: true,
            preferred_uid: 1000,
        });
        assert!(with_sudo.ends_with('\n'));
        assert!(with_sudo.contains("useradd -m -U -u"));
        assert!(with_sudo.contains("user.aibox.uid="));
        assert!(with_sudo.contains("90-aibox-user"));
        assert!(with_sudo.contains("aibox-user 'dev' 1000 1"));

        let without_sudo = create_user_snippet(&NewUserSpec {
            name: "dev".to_string(),
            sudo: false,
            preferred_uid: 1000,
        });
        assert!(without_sudo.contains("aibox-user 'dev' 1000 0"));
        assert!(without_sudo.contains("if [ \"$want_sudo\" = 1 ]"));
    }

    #[test]
    fn runtime_dir_owner_script_handles_present_and_absent_xdg_runtime_dir() {
        let user = LoginUser {
            name: "dev".to_string(),
            uid: 1000,
            gid: 1000,
        };
        assert_eq!(runtime_dir_owner_script("box", &user, &[]), "");
        let script = runtime_dir_owner_script(
            "box",
            &user,
            &[("XDG_RUNTIME_DIR".to_string(), "/run/user/1000".to_string())],
        );
        assert!(script.contains("chown \"$2:$3\" \"$1\""));
        assert!(script.contains("'/run/user/1000' 1000 1000"));
    }
}
