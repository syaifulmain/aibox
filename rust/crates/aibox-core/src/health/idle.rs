//! Idle detection helpers.

use std::collections::BTreeSet;

const VALUE_FLAGS: &[&str] = &[
    "--cwd",
    "--env",
    "--user",
    "--group",
    "--mode",
    "--project",
    "--target",
    "-e",
    "-u",
    "-g",
];

/// Parses `pgrep -a` output to find environment names with an active `incus exec` session.
pub fn parse_active_incus_exec_envs(pgrep_output: &str) -> BTreeSet<String> {
    let mut envs = BTreeSet::new();
    for raw_line in pgrep_output.split('\n') {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let Some(exec_idx) = line.find("incus exec ") else {
            continue;
        };
        let tail = line[exec_idx + 11..].trim();
        let tokens: Vec<&str> = tail.split_whitespace().collect();
        let mut i = 0;
        while i < tokens.len() {
            let tok = tokens[i];
            if tok == "--" {
                break;
            }
            if tok.starts_with('-') {
                if !tok.contains('=') && VALUE_FLAGS.contains(&tok) {
                    i += 1;
                }
                i += 1;
                continue;
            }
            envs.insert(tok.to_string());
            break;
        }
    }
    envs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_active_incus_exec_envs_with_flags() {
        let sample = "\
1001 incus exec --cwd /workspace -e FOO=bar --env BAZ=1 dev-box -- bash
1002 /usr/bin/incus exec --user 1000 --mode=interactive alpha-box
1003 incus exec -- dev-box
1004 incus shell other-box
1005 incus exec dev-box -- ls
";
        let got: Vec<String> = parse_active_incus_exec_envs(sample).into_iter().collect();
        assert_eq!(got, vec!["alpha-box", "dev-box"]);
    }
}
