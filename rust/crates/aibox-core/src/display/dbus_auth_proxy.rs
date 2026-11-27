//! D-Bus authentication proxy for containers.
//!
//! `xdg-dbus-proxy` forwards the credentials of the process that connects to
//! it. An Incus container connects through the host root process, so the host
//! bus sees UID 0 (or the proxy/root UID) and rejects the `AUTH EXTERNAL` line
//! that the container sent for its own UID. This proxy runs as the host user,
//! accepts the container connection, and rewrites the D-Bus auth exchange so
//! the bus authenticates the host user while `xdg-dbus-proxy` still applies the
//! name/path allowlist.
//!
//! The rewrite is intentionally narrow: only `AUTH EXTERNAL` and `DATA` lines
//! before `BEGIN` are replaced with the host UID encoded as hex of its ASCII
//! decimal form (`1000` becomes `31303030`). Everything after `BEGIN` is a raw
//! byte stream.

use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

const AUTH_PREFIX: &[u8] = b"AUTH EXTERNAL ";
const DATA_PREFIX: &[u8] = b"DATA ";
const NEGOTIATE_UNIX_FD: &[u8] = b"NEGOTIATE_UNIX_FD";
const SEPARATOR: &[u8] = b"\r\n";
const MAX_AUTH_PENDING_BYTES: usize = 16 * 1024;
const MAX_CONCURRENT_CONNECTIONS: usize = 64;

/// Encodes a numeric UID the way D-Bus expects for `AUTH EXTERNAL`.
///
/// D-Bus sends the ASCII decimal representation of the UID, hex encoded, so
/// UID 1000 becomes `31303030` (not `3e8`).
pub fn uid_auth_hex(uid: u32) -> String {
    let mut hex = String::with_capacity(8);
    for byte in uid.to_string().bytes() {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

/// Rewrites every complete auth line in `input`.
///
/// Returns the rewritten bytes, how many input bytes were consumed, and whether
/// `BEGIN` was reached. Bytes after `BEGIN` are intentionally left unconsumed so
/// the caller can forward them as a raw stream. An incomplete trailing line is
/// also left unconsumed.
pub fn rewrite_auth_lines(input: &[u8], uid_hex: &str) -> (Vec<u8>, usize, bool) {
    let (out, _, consumed, begin) = rewrite_auth_lines_with_local(input, uid_hex);
    (out, consumed, begin)
}

/// Rewrites auth lines, returning `(upstream_bytes, client_reply_bytes, consumed, begin)`.
/// `NEGOTIATE_UNIX_FD` is answered locally with `ERROR` so clients do not send
/// `SCM_RIGHTS` file descriptors that cannot be forwarded over plain streams.
fn rewrite_auth_lines_with_local(input: &[u8], uid_hex: &str) -> (Vec<u8>, Vec<u8>, usize, bool) {
    let mut out = Vec::with_capacity(input.len());
    let mut local_reply = Vec::new();
    let mut position = 0usize;
    let mut begin = false;

    while let Some(relative_end) = find_separator(&input[position..]) {
        let line_end = position + relative_end;
        let line = &input[position..line_end];
        let (prefix, body) = match line.first() {
            Some(0) => (&line[..1], &line[1..]),
            _ => (&line[..0], line),
        };

        if body == NEGOTIATE_UNIX_FD {
            if !prefix.is_empty() {
                out.extend_from_slice(prefix);
            }
            local_reply.extend_from_slice(b"ERROR \"Unix FD passing not supported\"\r\n");
            position = line_end + SEPARATOR.len();
            continue;
        }

        out.extend_from_slice(prefix);
        out.extend_from_slice(&rewrite_body(body, uid_hex));
        out.extend_from_slice(SEPARATOR);

        position = line_end + SEPARATOR.len();
        if body == b"BEGIN" {
            begin = true;
            break;
        }
    }

    (out, local_reply, position, begin)
}

fn rewrite_body(body: &[u8], uid_hex: &str) -> Vec<u8> {
    if body.starts_with(AUTH_PREFIX) {
        let mut rewritten = AUTH_PREFIX.to_vec();
        rewritten.extend_from_slice(uid_hex.as_bytes());
        return rewritten;
    }
    if body.starts_with(DATA_PREFIX) {
        let mut rewritten = DATA_PREFIX.to_vec();
        rewritten.extend_from_slice(uid_hex.as_bytes());
        return rewritten;
    }
    body.to_vec()
}

fn find_separator(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(SEPARATOR.len())
        .position(|window| window == SEPARATOR)
}

/// Extracts the filesystem path from a `unix:path=...` D-Bus address.
///
/// Only local `unix:path=` endpoints are supported; `unix:abstract=` and TCP
/// addresses return a descriptive error because a container cannot use them
/// through a filesystem Incus proxy.
pub fn parse_bus_path(address: &str) -> Result<String, String> {
    let mut saw_abstract = false;
    for entry in address.split(';') {
        let entry = entry.trim();
        let Some(rest) = entry.strip_prefix("unix:") else {
            continue;
        };
        let mut path = None;
        for part in rest.split(',') {
            if let Some(value) = part.strip_prefix("path=") {
                path = Some(percent_decode(value)?);
            } else if part.starts_with("abstract=") {
                saw_abstract = true;
            }
        }
        if let Some(path) = path.filter(|path| !path.is_empty()) {
            return Ok(path);
        }
    }
    if saw_abstract {
        return Err(
            "unsupported D-Bus address: unix:abstract endpoints are not supported; use unix:path"
                .to_string(),
        );
    }
    Err(format!(
        "unsupported D-Bus address '{address}': only unix:path=... is supported"
    ))
}

fn percent_decode(value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err(format!("invalid percent escape in '{value}'"));
            }
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3])
                .map_err(|_| format!("invalid percent escape in '{value}'"))?;
            let byte = u8::from_str_radix(hex, 16)
                .map_err(|_| format!("invalid percent escape in '{value}'"))?;
            decoded.push(byte);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).map_err(|_| format!("invalid UTF-8 in D-Bus address '{value}'"))
}

/// Runs the auth proxy until the process is killed.
///
/// `listen_path` is the socket the Incus proxy device connects to and
/// `bus_address` is the private socket of the filtered `xdg-dbus-proxy`.
pub fn run(listen_path: &str, bus_address: &str) -> i32 {
    let bus_path = match resolve_bus_path(bus_address) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("aibox --dbus-auth-proxy: {error}");
            return 2;
        }
    };
    match serve(listen_path, &bus_path) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("aibox --dbus-auth-proxy: {error}");
            1
        }
    }
}

/// Accepts either a D-Bus address (`unix:path=...`) or a raw socket path.
///
/// The bridge starts this mode with `unix:path=<filter.sock>`, while a manual
/// invocation may pass the path directly.
fn resolve_bus_path(address: &str) -> Result<String, String> {
    let trimmed = address.trim();
    if trimmed.starts_with('/') {
        return Ok(trimmed.to_string());
    }
    parse_bus_path(trimmed)
}

fn serve(listen_path: &str, bus_path: &str) -> io::Result<()> {
    match std::fs::remove_file(listen_path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    if let Some(parent) = Path::new(listen_path).parent() {
        if !parent.as_os_str().is_empty() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
        }
    }

    let listener = UnixListener::bind(listen_path)?;
    std::fs::set_permissions(listen_path, std::fs::Permissions::from_mode(0o600))?;

    struct ConnGuard(Arc<AtomicUsize>);
    impl Drop for ConnGuard {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::AcqRel);
        }
    }

    let active_conns = Arc::new(AtomicUsize::new(0));
    let uid_hex = uid_auth_hex(unsafe { libc::getuid() });
    for incoming in listener.incoming() {
        match incoming {
            Ok(client) => {
                let prev = active_conns.fetch_add(1, Ordering::AcqRel);
                if prev >= MAX_CONCURRENT_CONNECTIONS {
                    active_conns.fetch_sub(1, Ordering::AcqRel);
                    let _ = client.shutdown(std::net::Shutdown::Both);
                    continue;
                }
                let guard = ConnGuard(Arc::clone(&active_conns));
                let bus_path = bus_path.to_string();
                let uid_hex = uid_hex.clone();
                thread::spawn(move || {
                    let _guard = guard;
                    if let Err(error) = handle_client(client, &bus_path, &uid_hex) {
                        eprintln!("aibox --dbus-auth-proxy: client connection failed: {error}");
                    }
                });
            }
            Err(error) => {
                // EINTR is transient; every other accept error ends the loop.
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error);
            }
        }
    }
    Ok(())
}

fn handle_client(mut client: UnixStream, bus_path: &str, uid_hex: &str) -> io::Result<()> {
    let mut upstream = connect_upstream(bus_path)?;
    let mut upstream_read = upstream.try_clone()?;
    let mut client_write = client.try_clone()?;
    let mut client_read = client.try_clone()?;

    let bus_to_client = thread::spawn(move || {
        let mut buffer = [0u8; 65536];
        loop {
            match upstream_read.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    if client_write.write_all(&buffer[..read]).is_err() {
                        break;
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
        let _ = client_write.shutdown(std::net::Shutdown::Write);
    });

    let mut buffer = [0u8; 65536];
    let mut pending: Vec<u8> = Vec::new();
    let mut raw = false;
    let mut credentials_sent = false;
    let mut result: io::Result<()> = Ok(());

    loop {
        let read = match client_read.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => {
                result = Err(error);
                break;
            }
        };

        if raw {
            if let Err(error) = upstream.write_all(&buffer[..read]) {
                result = Err(error);
                break;
            }
            continue;
        }

        if pending.len().saturating_add(read) > MAX_AUTH_PENDING_BYTES {
            result = Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "D-Bus auth buffer limit exceeded",
            ));
            break;
        }
        pending.extend_from_slice(&buffer[..read]);
        let (out, local_reply, consumed, begin) = rewrite_auth_lines_with_local(&pending, uid_hex);
        if consumed > 0 {
            pending.drain(..consumed);
            if !out.is_empty() {
                let write_res = if !credentials_sent && out.first() == Some(&0) {
                    send_with_credentials(&upstream, &out)
                } else {
                    upstream.write_all(&out)
                };
                if let Err(error) = write_res {
                    result = Err(error);
                    break;
                }
                credentials_sent = true;
            }
            if !local_reply.is_empty() {
                if let Err(error) = client.write_all(&local_reply) {
                    result = Err(error);
                    break;
                }
            }
        }
        if begin {
            raw = true;
            if !pending.is_empty() {
                if let Err(error) = upstream.write_all(&pending) {
                    result = Err(error);
                    break;
                }
                pending.clear();
            }
        }
    }

    let _ = upstream.shutdown(std::net::Shutdown::Both);
    let _ = client.shutdown(std::net::Shutdown::Both);
    let _ = bus_to_client.join();
    result
}

/// Connects to the filtered proxy, retrying briefly while it starts.
fn connect_upstream(bus_path: &str) -> io::Result<UnixStream> {
    let mut last_error = None;
    for _ in 0..40 {
        match UnixStream::connect(bus_path) {
            Ok(stream) => return Ok(stream),
            Err(error) => {
                last_error = Some(error);
                thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    }
    Err(last_error.unwrap_or_else(|| io::Error::other("filtered proxy socket is missing")))
}

/// Sends the first auth chunk with explicit `SCM_CREDENTIALS` for the host user.
///
/// The leading NUL byte is how libdbus asks the peer to read the sender's
/// credentials. Sending them explicitly keeps the host bus from treating the
/// connection as the Incus proxy user.
fn send_with_credentials(mut stream: &UnixStream, data: &[u8]) -> io::Result<()> {
    use std::mem::size_of;

    #[repr(C)]
    struct ControlBuf {
        _align: [libc::cmsghdr; 0],
        buf: [u8; 64],
    }

    let credentials = libc::ucred {
        pid: unsafe { libc::getpid() },
        uid: unsafe { libc::getuid() },
        gid: unsafe { libc::getgid() },
    };
    let mut iov = libc::iovec {
        iov_base: data.as_ptr().cast_mut().cast(),
        iov_len: data.len(),
    };
    let control_len = unsafe { libc::CMSG_SPACE(size_of::<libc::ucred>() as u32) } as usize;
    let mut control = ControlBuf {
        _align: [],
        buf: [0u8; 64],
    };
    if control_len > control.buf.len() {
        return Err(io::Error::other("SCM_CREDENTIALS control buffer too small"));
    }
    let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
    message.msg_iov = &mut iov;
    message.msg_iovlen = 1;
    message.msg_control = control.buf.as_mut_ptr().cast();
    message.msg_controllen = control_len;

    unsafe {
        let header = libc::CMSG_FIRSTHDR(&message);
        if header.is_null() {
            return Err(io::Error::other("could not build SCM_CREDENTIALS header"));
        }
        (*header).cmsg_level = libc::SOL_SOCKET;
        (*header).cmsg_type = libc::SCM_CREDENTIALS;
        (*header).cmsg_len = libc::CMSG_LEN(size_of::<libc::ucred>() as u32) as _;
        std::ptr::copy_nonoverlapping(
            (&credentials as *const libc::ucred).cast::<u8>(),
            libc::CMSG_DATA(header),
            size_of::<libc::ucred>(),
        );
    }

    let sent = loop {
        let rc = unsafe { libc::sendmsg(stream.as_raw_fd(), &message, libc::MSG_NOSIGNAL) };
        if rc < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(err);
        }
        break rc as usize;
    };
    if sent == 0 && !data.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::WriteZero,
            "sendmsg wrote zero bytes",
        ));
    }
    if sent < data.len() {
        stream.write_all(&data[sent..])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(bytes: &[u8]) -> String {
        String::from_utf8_lossy(bytes).to_string()
    }

    #[test]
    fn uid_hex_encodes_ascii_decimal_not_the_number() {
        assert_eq!(uid_auth_hex(1000), "31303030");
        assert_eq!(uid_auth_hex(0), "30");
        assert_eq!(uid_auth_hex(4242), "34323432");
    }

    #[test]
    fn rewrites_auth_data_and_keeps_other_lines() {
        let input = b"\0AUTH EXTERNAL 30\r\nDATA\r\nHELLO\r\nBEGIN\r\n";
        let (out, consumed, begin) = rewrite_auth_lines(input, "31303030");
        assert!(begin);
        assert_eq!(consumed, input.len());
        assert_eq!(
            text(&out),
            "\0AUTH EXTERNAL 31303030\r\nDATA\r\nHELLO\r\nBEGIN\r\n"
        );
    }

    #[test]
    fn rewrites_data_with_and_without_a_value() {
        for input in [b"DATA 31303030\r\n".as_slice(), b"DATA \r\n".as_slice()] {
            let (out, consumed, begin) = rewrite_auth_lines(input, "31303030");
            assert_eq!(consumed, input.len());
            assert!(!begin);
            assert_eq!(text(&out), "DATA 31303030\r\n");
        }
    }

    #[test]
    fn leaves_an_incomplete_line_buffered() {
        let input = b"\0AUTH EXTERNAL 3130";
        let (out, consumed, begin) = rewrite_auth_lines(input, "31303030");
        assert!(out.is_empty());
        assert_eq!(consumed, 0);
        assert!(!begin);
    }

    #[test]
    fn handles_auth_lines_without_the_leading_nul() {
        let (out, consumed, begin) = rewrite_auth_lines(b"AUTH EXTERNAL 30\r\n", "31303030");
        assert_eq!(consumed, 18);
        assert!(!begin);
        assert_eq!(text(&out), "AUTH EXTERNAL 31303030\r\n");
    }

    #[test]
    fn preserves_bytes_after_begin_for_the_raw_stream() {
        let input = b"\0AUTH EXTERNAL 30\r\nBEGIN\r\nmethod-call-bytes";
        let (out, consumed, begin) = rewrite_auth_lines(input, "31303030");
        assert!(begin);
        assert_eq!(consumed, 26);
        assert_eq!(text(&out), "\0AUTH EXTERNAL 31303030\r\nBEGIN\r\n");
        assert_eq!(&input[consumed..], b"method-call-bytes");
    }

    #[test]
    fn non_auth_lines_and_complete_prefix_lines_pass_through() {
        let input = b"\0AUTH EXTERNAL 30\r\nHELLO\r\n";
        let (_, consumed, begin) = rewrite_auth_lines(input, "31303030");
        assert_eq!(consumed, input.len());
        assert!(!begin);

        let (out, _, _) = rewrite_auth_lines(b"HELLO\r\nERROR\r\n", "31303030");
        assert_eq!(text(&out), "HELLO\r\nERROR\r\n");
    }

    #[test]
    fn negotiate_unix_fd_is_rejected_locally() {
        let input = b"\0AUTH EXTERNAL 30\r\nNEGOTIATE_UNIX_FD\r\nBEGIN\r\n";
        let (out, local_reply, consumed, begin) = rewrite_auth_lines_with_local(input, "31303030");
        assert_eq!(consumed, input.len());
        assert!(begin);
        assert_eq!(text(&out), "\0AUTH EXTERNAL 31303030\r\nBEGIN\r\n");
        assert_eq!(
            text(&local_reply),
            "ERROR \"Unix FD passing not supported\"\r\n"
        );
    }

    #[test]
    fn parses_path_addresses_and_percent_escapes() {
        assert_eq!(
            parse_bus_path("unix:path=/run/user/1000/bus").unwrap(),
            "/run/user/1000/bus"
        );
        assert_eq!(
            parse_bus_path("unix:path=/run/user/1000/aibox%2Ddbus/x;socket").unwrap(),
            "/run/user/1000/aibox-dbus/x"
        );
        assert_eq!(
            parse_bus_path("unix:guid=abc,path=/tmp/bus").unwrap(),
            "/tmp/bus"
        );
    }

    #[test]
    fn rejects_addresses_a_container_proxy_cannot_use() {
        for address in [
            "tcp:host=127.0.0.1,port=4444",
            "unix:abstract=/tmp/dbus-test",
            "unix:",
            "",
            "unix:path=/tmp/%zz",
        ] {
            assert!(
                parse_bus_path(address).is_err(),
                "address should be rejected: {address}"
            );
        }
    }

    #[test]
    fn accepts_raw_absolute_socket_paths_for_manual_runs() {
        assert_eq!(
            resolve_bus_path("/run/user/1000/aibox-dbus/box/filter.sock").unwrap(),
            "/run/user/1000/aibox-dbus/box/filter.sock"
        );
        assert_eq!(
            resolve_bus_path("unix:path=/run/user/1000/bus").unwrap(),
            "/run/user/1000/bus"
        );
        assert!(resolve_bus_path("tcp:host=127.0.0.1,port=4444").is_err());
    }
}
