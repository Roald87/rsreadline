//! `rsreadline --update`: replace the running binary with the latest
//! GitHub release.
//!
//! Zero-dependency, so `curl` does the HTTPS — one call to the releases API
//! for the latest tag, one to download the asset, skipped when already
//! current. The `rename` over `current_exe()` is safe because Linux keeps
//! the open inode alive for the running process.

use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::Command;

const REPO: &str = "Roald87/rsreadline";
const ASSET: &str = "rsreadline-x86_64-unknown-linux-gnu";

/// Downloads and installs the latest release over the current binary.
/// `force` re-installs even when the versions match. Returns a
/// user-facing message on failure.
pub fn run(force: bool) -> Result<(), String> {
    if std::env::consts::OS != "linux" || std::env::consts::ARCH != "x86_64" {
        return Err(format!(
            "no prebuilt binary for {}-{}; build from source instead",
            std::env::consts::OS,
            std::env::consts::ARCH,
        ));
    }

    let current = env!("CARGO_PKG_VERSION");
    let latest_tag = fetch_latest_tag()?;
    let latest = latest_tag.trim_start_matches('v');

    if !force && !is_newer(latest, current) {
        println!("rsreadline {current} is already the latest release");
        return Ok(());
    }

    let exe =
        std::env::current_exe().map_err(|e| format!("can't locate the running binary: {e}"))?;
    let mut tmp_name = exe.file_name().map(OsString::from).unwrap_or_default();
    tmp_name.push(".update.tmp");
    let tmp = exe.with_file_name(tmp_name);

    let install = download(&tmp)
        .and_then(|()| make_executable(&tmp))
        .and_then(|()| std::fs::rename(&tmp, &exe));
    if let Err(e) = install {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("can't install to {}: {e}", exe.display()));
    }

    println!("Updated rsreadline {current} -> {latest}");
    println!("Restart bash to load the new binary.");
    Ok(())
}

fn fetch_latest_tag() -> Result<String, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let body = curl_stdout(&["-fsSL", &url]).map_err(|e| format!("can't reach GitHub: {e}"))?;
    parse_tag_name(&body).ok_or_else(|| "no tag_name in the GitHub response".to_string())
}

/// Pulls the value of the first `"tag_name"` key out of the releases-API
/// JSON without a real parser — the response shape is fixed.
fn parse_tag_name(json: &str) -> Option<String> {
    let after_key = json.split_once("\"tag_name\"")?.1;
    let after_colon = after_key.split_once(':')?.1;
    let open = after_colon.find('"')? + 1;
    let close = after_colon[open..].find('"')? + open;
    Some(after_colon[open..close].to_string())
}

fn download(dest: &Path) -> io::Result<()> {
    let url = format!("https://github.com/{REPO}/releases/latest/download/{ASSET}");
    let status = Command::new("curl")
        .args(["-fsSL", &url, "-o"])
        .arg(dest)
        .status()
        .map_err(curl_io_err)?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("curl exited with {status}")))
    }
}

fn make_executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(perms.mode() | 0o755);
    std::fs::set_permissions(path, perms)
}

fn curl_stdout(args: &[&str]) -> io::Result<String> {
    let out = Command::new("curl")
        .args(args)
        .output()
        .map_err(curl_io_err)?;
    if !out.status.success() {
        return Err(io::Error::other(format!(
            "curl exited with {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim(),
        )));
    }
    String::from_utf8(out.stdout).map_err(|_| io::Error::other("non-UTF-8 response"))
}

fn curl_io_err(e: io::Error) -> io::Error {
    if e.kind() == io::ErrorKind::NotFound {
        io::Error::other("curl is not on PATH (rsreadline --update needs it)")
    } else {
        e
    }
}

fn is_newer(candidate: &str, current: &str) -> bool {
    version_parts(candidate) > version_parts(current)
}

fn version_parts(v: &str) -> Vec<u64> {
    v.split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tag_name_reads_the_release_tag() {
        let json = r#"{"url":"...","tag_name":"v0.5.0","name":"0.5.0"}"#;
        assert_eq!(parse_tag_name(json).as_deref(), Some("v0.5.0"));
    }

    #[test]
    fn parse_tag_name_is_none_when_absent() {
        assert_eq!(parse_tag_name(r#"{"name":"0.5.0"}"#), None);
    }

    #[test]
    fn is_newer_compares_numerically_not_lexically() {
        assert!(is_newer("0.10.0", "0.9.0"));
        assert!(is_newer("0.4.2", "0.4.1"));
        assert!(is_newer("1.0.0", "0.99.0"));
    }

    #[test]
    fn is_newer_is_false_for_same_or_older() {
        assert!(!is_newer("0.4.1", "0.4.1"));
        assert!(!is_newer("0.4.0", "0.4.1"));
    }
}
