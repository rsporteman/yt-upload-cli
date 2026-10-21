//! Environment variables persisted by the operating system ("global" variables). A terminal,
//! scheduler or agent started before such a variable was set does not have it in its process
//! environment, so the CLI reads the OS definition as a fallback.

use std::path::Path;

/// A variable found in the OS-persisted environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Persisted {
    pub value: String,
    /// Where it was found, for error messages (e.g. "/etc/environment").
    pub source: &'static str,
}

/// Looks `name` up in the environment the OS persists for the current user, then for the system.
/// Unreadable sources count as empty: a missing fallback must never break a command.
#[cfg(windows)]
pub fn os_lookup(name: &str) -> Option<Persisted> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

    const SYSTEM: &str = r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment";
    [
        (
            HKEY_CURRENT_USER,
            "Environment",
            "the Windows user environment",
        ),
        (HKEY_LOCAL_MACHINE, SYSTEM, "the Windows system environment"),
    ]
    .into_iter()
    .find_map(|(root, path, source)| {
        let raw = read_registry(&RegKey::predef(root), path, name)?;
        let value = expand_percent(&raw, |v| std::env::var(v).ok());
        (!value.is_empty()).then_some(Persisted { value, source })
    })
}

/// Raw value of `name` under `root\path`, for both REG_SZ and REG_EXPAND_SZ.
#[cfg(windows)]
fn read_registry(root: &winreg::RegKey, path: &str, name: &str) -> Option<String> {
    root.open_subkey(path).ok()?.get_value(name).ok()
}

#[cfg(target_os = "macos")]
pub fn os_lookup(name: &str) -> Option<Persisted> {
    let out = std::process::Command::new("launchctl")
        .args(["getenv", name])
        .output()
        .ok()?;
    let value = String::from_utf8(out.stdout).ok()?.trim_end().to_owned();
    (out.status.success() && !value.is_empty()).then_some(Persisted {
        value,
        source: "the launchd user environment",
    })
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn os_lookup(name: &str) -> Option<Persisted> {
    let user_dir = dirs::config_dir().map(|d| d.join("environment.d"));
    lookup_env_dirs(name, user_dir.as_deref(), Path::new("/etc/environment"))
}

#[cfg(not(any(windows, unix)))]
pub fn os_lookup(_name: &str) -> Option<Persisted> {
    None
}

/// Looks `name` up in the `*.conf` files of `user_dir` (systemd `environment.d`; later files
/// win), then in `system_file` (`/etc/environment` format).
pub fn lookup_env_dirs(
    name: &str,
    user_dir: Option<&Path>,
    system_file: &Path,
) -> Option<Persisted> {
    let mut confs: Vec<_> = user_dir
        .and_then(|d| std::fs::read_dir(d).ok())
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "conf"))
        .collect();
    confs.sort();
    let from_file = |path: &Path| {
        let text = std::fs::read_to_string(path).ok()?;
        parse_env_file(&text)
            .into_iter()
            .filter(|(k, _)| k == name)
            .map(|(_, v)| v)
            .next_back()
    };
    let found = |value: Option<String>, source| {
        value
            .filter(|v| !v.is_empty())
            .map(|value| Persisted { value, source })
    };
    found(
        confs.iter().filter_map(|p| from_file(p)).next_back(),
        "~/.config/environment.d",
    )
    .or_else(|| found(from_file(system_file), "/etc/environment"))
}

/// `KEY=VALUE` pairs of an environment file. Blank lines, `#` comments and lines without `=`
/// are skipped; a leading `export ` and one pair of matching quotes around the value are removed.
pub fn parse_env_file(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.strip_prefix("export ").unwrap_or(l).split_once('='))
        .map(|(k, v)| (k.trim().to_owned(), unquote(v.trim()).to_owned()))
        .collect()
}

fn unquote(v: &str) -> &str {
    for q in ['"', '\''] {
        if let Some(inner) = v.strip_prefix(q).and_then(|v| v.strip_suffix(q)) {
            return inner;
        }
    }
    v
}

/// Expands Windows-style `%NAME%` references with `get`; unknown names stay as written, as
/// Windows itself does.
pub fn expand_percent(value: &str, get: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::new();
    let mut rest = value;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) if end > 0 => match get(&after[..end]) {
                Some(v) => {
                    out.push_str(&v);
                    rest = &after[end + 1..];
                }
                None => {
                    out.push('%');
                    rest = after;
                }
            },
            _ => {
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_env_files() {
        let text =
            "\n# comment\nA=1\nexport B = \"two words\"\nC='x'\nD=\"unbalanced'\nno-equals\nE=\n";
        assert_eq!(
            parse_env_file(text),
            [
                ("A", "1"),
                ("B", "two words"),
                ("C", "x"),
                ("D", "\"unbalanced'"),
                ("E", ""),
            ]
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
        );
    }

    #[test]
    fn user_conf_files_override_system_file() {
        let dir = tempfile::tempdir().unwrap();
        let user = dir.path().join("environment.d");
        std::fs::create_dir(&user).unwrap();
        std::fs::write(user.join("10-a.conf"), "X=first\nY=\n").unwrap();
        std::fs::write(user.join("20-b.conf"), "X=second\n").unwrap();
        std::fs::write(user.join("30-c.txt"), "X=ignored\n").unwrap();
        let system = dir.path().join("environment");
        std::fs::write(&system, "X=system\nY=system-y\nZ=system-z\n").unwrap();

        let get = |name| lookup_env_dirs(name, Some(&user), &system);
        assert_eq!(
            get("X"),
            Some(Persisted {
                value: "second".into(),
                source: "~/.config/environment.d"
            })
        );
        // An empty user value counts as unset, so the system value applies.
        assert_eq!(get("Y").unwrap().value, "system-y");
        assert_eq!(get("Z").unwrap().source, "/etc/environment");
        assert_eq!(get("MISSING"), None);
    }

    #[test]
    fn missing_env_sources_find_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let none = dir.path().join("none");
        assert_eq!(lookup_env_dirs("X", Some(&none), &none), None);
        assert_eq!(lookup_env_dirs("X", None, &none), None);
    }

    #[test]
    fn expands_percent_references() {
        let get = |n: &str| (n == "HOME").then(|| "C:\\Users\\me".to_owned());
        assert_eq!(expand_percent("%HOME%\\yt", get), "C:\\Users\\me\\yt");
        assert_eq!(
            expand_percent("%NOPE%\\%HOME%", get),
            "%NOPE%\\C:\\Users\\me"
        );
        assert_eq!(expand_percent("100% sure %% x", get), "100% sure %% x");
        assert_eq!(expand_percent("plain", get), "plain");
    }

    #[test]
    fn os_lookup_ignores_unknown_variables() {
        assert_eq!(os_lookup("YT_UPLOAD_CLI_SURELY_UNSET_12345"), None);
    }

    #[cfg(windows)]
    #[test]
    fn reads_and_expands_registry_values() {
        use winreg::RegKey;
        use winreg::enums::{HKEY_CURRENT_USER, RegType};

        struct Cleanup(String);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(&self.0);
            }
        }

        let path = format!(r"Software\yt-upload-cli-test-{}", std::process::id());
        let _cleanup = Cleanup(path.clone());
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu.create_subkey(&path).unwrap();
        key.set_value("PLAIN", &"value").unwrap();
        let expand = winreg::RegValue {
            bytes: "%SystemRoot%\\x\0"
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect(),
            vtype: RegType::REG_EXPAND_SZ,
        };
        key.set_raw_value("EXPAND", &expand).unwrap();

        assert_eq!(
            read_registry(&hkcu, &path, "PLAIN").as_deref(),
            Some("value")
        );
        let raw = read_registry(&hkcu, &path, "EXPAND").unwrap();
        assert_eq!(raw, "%SystemRoot%\\x");
        let expanded = expand_percent(&raw, |v| std::env::var(v).ok());
        assert!(!expanded.contains('%') && expanded.ends_with("\\x"));
        assert_eq!(read_registry(&hkcu, &path, "MISSING"), None);
        assert_eq!(
            read_registry(&hkcu, r"Software\no-such-key-xyz", "PLAIN"),
            None
        );
    }
}
