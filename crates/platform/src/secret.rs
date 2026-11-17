//! Keeping a secret (an AI provider's API key) in a settings file without writing it in clear
//! text (AJS fork).
//!
//! On Windows the secret is encrypted with DPAPI for the current Windows account: only that account,
//! on that computer, can read it back, so a copied settings file or portable folder doesn't carry
//! a usable key. The encryption is Windows' own, reached through PowerShell's
//! `ConvertFrom-SecureString` (as `pdf_open_with` reaches the registry), so this crate stays free of
//! `unsafe`. The secret travels over the child's standard input, never its command line or
//! environment, and nothing here logs it.
//!
//! Elsewhere there is no such store here: [`protect`] returns an error and the caller keeps the key
//! for the session only (or the user sets the provider's environment variable).

/// Marks a value [`protect`] wrote.
const PREFIX: &str = "dpapi:";
/// Longest secret accepted; API keys are a few hundred characters at most.
const MAX_SECRET: usize = 4096;

/// `secret`, encrypted for this Windows account, as text to keep in a settings file.
pub fn protect(secret: &str) -> std::io::Result<String> {
    if secret.is_empty() || secret.len() > MAX_SECRET || secret.contains(['\r', '\n', '\0']) {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "the key must be one line of at most 4096 characters"));
    }
    let blob = imp::run(imp::PROTECT, secret)?;
    let blob = blob.trim();
    if blob.is_empty() || !blob.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(std::io::Error::other("Windows returned no encrypted key"));
    }
    Ok(format!("{PREFIX}{blob}"))
}

/// The secret a [`protect`] value holds. Fails for a value another account or computer wrote.
pub fn unprotect(stored: &str) -> std::io::Result<String> {
    let Some(blob) = stored.strip_prefix(PREFIX).filter(|b| !b.is_empty() && b.bytes().all(|c| c.is_ascii_hexdigit())) else {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "not a saved key"));
    };
    let secret = imp::run(imp::UNPROTECT, blob)?;
    // PowerShell may end its output with a line break; a key never contains one.
    Ok(secret.trim_end_matches(['\r', '\n']).to_owned())
}

#[cfg(windows)]
mod imp {
    use std::io::Write as _;
    use std::os::windows::process::CommandExt as _;
    use std::path::PathBuf;
    use std::process::Stdio;

    /// `CREATE_NO_WINDOW`: no console window flashes up.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    /// Reads the secret from standard input and writes DPAPI's encryption of it as hex.
    pub(super) const PROTECT: &str = r#"$ErrorActionPreference = 'Stop'
[Console]::InputEncoding = [Text.Encoding]::UTF8
$s = [Console]::In.ReadToEnd()
$ss = ConvertTo-SecureString -String $s -AsPlainText -Force
[Console]::Out.Write((ConvertFrom-SecureString -SecureString $ss))"#;

    /// Reads the hex from standard input and writes the secret.
    pub(super) const UNPROTECT: &str = r#"$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$b = [Console]::In.ReadToEnd().Trim()
$ss = ConvertTo-SecureString -String $b
[Console]::Out.Write(([Net.NetworkCredential]::new('', $ss)).Password)"#;

    pub(super) fn run(script: &str, input: &str) -> std::io::Result<String> {
        let system = std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
        // By full path: a `powershell.exe` next to the app or earlier on the search path must not answer.
        let mut child = std::process::Command::new(system.join(r"System32\WindowsPowerShell\v1.0\powershell.exe"))
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input.as_bytes())?;
        }
        let out = child.wait_with_output()?;
        if !out.status.success() {
            // The message names the failure only; the input is never part of it.
            let err = String::from_utf8_lossy(&out.stderr);
            let first = err.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("PowerShell failed");
            return Err(std::io::Error::other(format!("Windows couldn't protect or read the key: {first}")));
        }
        String::from_utf8(out.stdout).map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "the key isn't text"))
    }
}

#[cfg(not(windows))]
mod imp {
    pub(super) const PROTECT: &str = "";
    pub(super) const UNPROTECT: &str = "";

    pub(super) fn run(_script: &str, _input: &str) -> std::io::Result<String> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "saving keys encrypted needs Windows; set the provider's environment variable instead",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bad_input_is_refused_before_anything_runs() {
        assert!(protect("").is_err());
        assert!(protect("two\nlines").is_err());
        assert!(protect(&"x".repeat(MAX_SECRET + 1)).is_err());
        assert!(unprotect("sk-plain-key").is_err(), "a clear-text value is never taken as saved");
        assert!(unprotect("dpapi:").is_err());
        assert!(unprotect("dpapi:zz").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn a_key_comes_back_only_from_its_encrypted_form() {
        let key = "sk-test-Ảnh-ключ-1234567890";
        let stored = protect(key).unwrap();
        assert!(stored.starts_with(PREFIX));
        assert!(!stored.contains("sk-test"), "the key is not in the stored text");
        assert_eq!(unprotect(&stored).unwrap(), key);
        // A damaged value is an error, not a wrong key.
        let mut damaged = stored.clone();
        damaged.truncate(stored.len() - 8);
        assert!(unprotect(&damaged).is_err());
    }
}
