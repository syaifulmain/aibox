//! Small, readable shell script templates and rendering helpers for provisioning.

/// Replaces `{{KEY}}` placeholders in `template` with the provided values.
pub(crate) fn render_template(template: &str, vars: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (key, val) in vars {
        let placeholder = format!("{{{{{key}}}}}");
        out = out.replace(&placeholder, val);
    }
    out
}

/// Generates a structured reverse-order rollback function and EXIT trap for `target_var` (`NAME` or `DST`).
pub(crate) fn rollback_trap_snippet(target_var: &str) -> String {
    render_template(
        r#"AIBOX_DONE=0
ROLLBACK_WS=0
ROLLBACK_INSTANCE=0

aibox_rollback() {
    local rc="$1"
    set +e
    trap - EXIT
    echo "[rollback] Provisioning failed (exit $rc); cleaning up completed steps..." >&2
    if [ "${ROLLBACK_INSTANCE:-0}" = "1" ]; then
        incus delete "${{TARGET_VAR}}" --force >/dev/null 2>&1 || true
        incus network acl delete "aibox-${{TARGET_VAR}}" >/dev/null 2>&1 || true
        incus network acl delete "aibox-log-${{TARGET_VAR}}" >/dev/null 2>&1 || true
        echo "[rollback] Removed instance and network ACLs for '${{TARGET_VAR}}'." >&2
    fi
    if [ "${ROLLBACK_WS:-0}" = "1" ] && [ -e "$WORKSPACE" ]; then
        if command -v btrfs >/dev/null 2>&1 && [[ "$(stat -f -c %T "$WORKSPACE" 2>/dev/null || true)" == "btrfs" ]] && [[ "$(stat -c %i "$WORKSPACE" 2>/dev/null || true)" == "256" ]]; then
            btrfs property set -ts "$WORKSPACE" ro false >/dev/null 2>&1 || true
            btrfs subvolume delete "$WORKSPACE" >/dev/null 2>&1 || rm -rf "$WORKSPACE" >/dev/null 2>&1 || true
        else
            rm -rf "$WORKSPACE" >/dev/null 2>&1 || true
        fi
        echo "[rollback] Removed workspace '$WORKSPACE'." >&2
    fi
    exit "$rc"
}
trap 'rc=$?; if [ "$rc" -ne 0 ] && [ "${AIBOX_DONE:-0}" != "1" ]; then aibox_rollback "$rc"; fi' EXIT"#,
        &[("TARGET_VAR", target_var)],
    )
}

pub(crate) const CREATE_SCRIPT_TEMPLATE: &str = r#"
set -euo pipefail
NAME={{NAME}}
LOCAL_ALIAS={{LOCAL_ALIAS}}
REMOTE_IMAGE={{REMOTE_IMAGE}}
WORKSPACE={{WORKSPACE}}
WS_PATH={{WS_PATH}}
CACHE={{CACHE}}
CACHE_ROOT={{CACHE_ROOT}}
PROFILE_ARGS=({{PROFILE_ARGS}})
QUOTA_ARGS=({{QUOTA_ARGS}})
X11={{X11}}

{{ROLLBACK_TRAP}}

echo "@@STAGE:8:Step 1/4 — Preparing shared workspace directory..."
if [ ! -e "$WORKSPACE" ]; then
    WS_EXISTED=0
else
    WS_EXISTED=1
fi
{{WORKSPACE_CREATE}}
if [ "$WS_EXISTED" = "0" ] && [ -e "$WORKSPACE" ]; then
    ROLLBACK_WS=1
fi
if command -v setfacl >/dev/null 2>&1; then
    ACL_USER="${SUDO_USER:-${USER:-$(id -un)}}"
    setfacl -R -m "u:${ACL_USER}:rwx" -m "m::rwx" -d -m "u:${ACL_USER}:rwx" -d -m "m::rwx" "$WORKSPACE" 2>/dev/null || true
fi

if incus image info "$LOCAL_ALIAS" >/dev/null 2>&1; then
    echo "@@STAGE:60:Step 2/4 — Using cached OS image ($LOCAL_ALIAS)..."
    SOURCE_IMAGE="$LOCAL_ALIAS"
elif [ -s "$CACHE" ] && [ -s "$CACHE_ROOT" ]; then
    echo "@@STAGE:30:Step 2/4 — Importing OS image from OS storage..."
    incus image import "$CACHE" "$CACHE_ROOT" --alias "$LOCAL_ALIAS"
    SOURCE_IMAGE="$LOCAL_ALIAS"
else
    echo "@@STAGE:12:Step 2/4 — Downloading OS image ($REMOTE_IMAGE)..."
    script -q -e -c "incus image copy \"$REMOTE_IMAGE\" local: --alias \"$LOCAL_ALIAS\" --auto-update" /dev/null
    SOURCE_IMAGE="$LOCAL_ALIAS"
fi

echo "@@STAGE:72:Step 3/4 — Launching isolated OS container & applying quotas..."
script -q -e -c "incus launch \"$SOURCE_IMAGE\" \"$NAME\" ${PROFILE_ARGS[*]} ${QUOTA_ARGS[*]}" /dev/null
ROLLBACK_INSTANCE=1

echo "@@STAGE:86:Step 4/4 — Mounting $WS_PATH & waiting for network IP..."
sleep 1

if ! incus config device add "$NAME" workspace disk source="$WORKSPACE" path="$WS_PATH" shift=true >/dev/null 2>&1; then
    incus config device add "$NAME" workspace disk source="$WORKSPACE" path="$WS_PATH" >/dev/null
fi

if [[ "$X11" == "1" ]] && [[ -d /tmp/.X11-unix ]]; then
    if command -v xhost >/dev/null 2>&1 && [[ -n "${DISPLAY:-}" ]]; then
        xhost +local: >/dev/null 2>&1 || true
    fi
    incus config device add "$NAME" x11 disk source=/tmp/.X11-unix path=/tmp/.X11-unix >/dev/null 2>&1 || true
fi

incus exec "$NAME" -- sh -c '
    mkdir -p "$1" && chmod 777 "$1" || true
    if [ -f /etc/dhcpcd.conf ] && ! grep -q "^noarp" /etc/dhcpcd.conf; then
        echo "noarp" >> /etc/dhcpcd.conf
        pkill -HUP dhcpcd 2>/dev/null || udhcpc -i eth0 -n -q 2>/dev/null || true
    fi
' aibox "$WS_PATH" >/dev/null 2>&1 || true

for _ in 1 2 3 4 5; do
    if incus list "$NAME" -f csv -c 4 | grep -q "[0-9]"; then
        break
    fi
    sleep 1
done

AIBOX_DONE=1
echo "@@STAGE:100:Environment ready!"
"#;

pub(crate) const CLONE_SCRIPT_TEMPLATE: &str = r#"
set -euo pipefail
SRC={{SRC}}
DST={{DST}}
WORKSPACE={{WORKSPACE}}
SRC_WORKSPACE={{SRC_WORKSPACE}}
WS_PATH={{WS_PATH}}

{{ROLLBACK_TRAP}}

echo "@@STAGE:12:Step 1/3 — Preparing fresh workspace directory..."
if [ ! -e "$WORKSPACE" ]; then
    WS_EXISTED=0
else
    WS_EXISTED=1
fi
if [ -n "$SRC_WORKSPACE" ] && [ -e "$SRC_WORKSPACE" ] && [ ! -e "$WORKSPACE" ] \
    && command -v btrfs >/dev/null 2>&1 \
    && [[ "$(stat -f -c %T "$SRC_WORKSPACE" 2>/dev/null || true)" == "btrfs" ]] \
    && [[ "$(stat -c %i "$SRC_WORKSPACE" 2>/dev/null || true)" == "256" ]] \
    && [[ "$(stat -f -c %T "$(dirname "$WORKSPACE")" 2>/dev/null || true)" == "btrfs" ]]; then
    mkdir -p "$(dirname "$WORKSPACE")"
    btrfs subvolume snapshot "$SRC_WORKSPACE" "$WORKSPACE" >/dev/null
else
    {{WORKSPACE_CREATE}}
fi
if [ "$WS_EXISTED" = "0" ] && [ -e "$WORKSPACE" ]; then
    ROLLBACK_WS=1
fi
if command -v setfacl >/dev/null 2>&1; then
    ACL_USER="${SUDO_USER:-${USER:-$(id -un)}}"
    setfacl -R -m "u:${ACL_USER}:rwx" -m "m::rwx" -d -m "u:${ACL_USER}:rwx" -d -m "m::rwx" "$WORKSPACE" 2>/dev/null || true
fi

echo "@@STAGE:35:Step 2/3 — Cloning environment with Btrfs copy-on-write..."
script -q -e -c "incus copy \"$SRC\" \"$DST\" --instance-only --stateless" /dev/null
ROLLBACK_INSTANCE=1

echo "@@STAGE:70:Step 3/3 — Configuring isolated workspace & limits..."
incus config unset "$DST" user.aibox.template >/dev/null 2>&1 || true
incus config device remove "$DST" workspace >/dev/null 2>&1 || true
if ! incus config device add "$DST" workspace disk source="$WORKSPACE" path="$WS_PATH" shift=true >/dev/null 2>&1; then
    incus config device add "$DST" workspace disk source="$WORKSPACE" path="$WS_PATH" >/dev/null
fi
{{QUOTA_LINES}}
{{NET_SNIPPET}}
{{START_SNIPPET}}
AIBOX_DONE=1
echo "@@STAGE:100:Clone ready!"
"#;

pub(crate) const PREFETCH_IMAGE_TEMPLATE: &str = r#"
set -euo pipefail
LOCAL_ALIAS={{LOCAL_ALIAS}}
REMOTE_IMAGE={{REMOTE_IMAGE}}
CACHE={{CACHE}}
CACHE_ROOT={{CACHE_ROOT}}

mkdir -p "$(dirname "$CACHE")"
if incus image info "$LOCAL_ALIAS" >/dev/null 2>&1; then
    exit 0
elif [ -s "$CACHE" ] && [ -s "$CACHE_ROOT" ]; then
    incus image import "$CACHE" "$CACHE_ROOT" --alias "$LOCAL_ALIAS" >/dev/null 2>&1 || true
    exit 0
fi

incus image copy "$REMOTE_IMAGE" local: --alias "$LOCAL_ALIAS" --auto-update >/dev/null 2>&1 || exit 0
if [ ! -s "$CACHE" ] || [ ! -s "$CACHE_ROOT" ]; then
    rm -f "$CACHE" "$CACHE_ROOT"
    incus image export "$LOCAL_ALIAS" "$CACHE" >/dev/null 2>&1 || true
fi
"#;
