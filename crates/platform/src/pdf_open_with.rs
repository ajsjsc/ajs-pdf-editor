//! AJS fork: offer AJS PDF Editor for PDF files in Windows' "Open with" list and in Settings ▸
//! Apps ▸ Default apps (Help ▸ Add to "Open with" for PDF files), and take it back out again.
//!
//! Windows only, for the current user (`HKEY_CURRENT_USER`), so it needs no administrator rights
//! and works for every copy: the AJS installer's, the one the AJS AutoCAD plug-in (D2P) installs and
//! the portable zip. It follows Windows' Default Programs registration, the same layout the
//! upstream PdfCraft installer and its portable notes use: a ProgID, `.pdf\OpenWithProgids`,
//! `Applications\<exe>`, Capabilities and `RegisteredApplications`. It never takes over the
//! default: `.pdf`'s own default and the user's choice are left alone, and the user picks AJS PDF
//! Editor in "Open with" or Default apps. Registering again points everything at the copy that
//! runs it.
//!
//! The values are written by .NET's registry API through the system PowerShell, so this needs no
//! `unsafe` code. The paths reach the script through environment variables, never through the
//! script text, so no file name can change what the script does.

use std::path::Path;

/// The ProgID the `.pdf` "Open with" entry names.
pub const PROG_ID: &str = "AJSPdfEditor.Document";

/// The name Windows shows in "Open with" and Default apps.
pub const APP_NAME: &str = "AJS PDF Editor";

/// The app's description in Default apps, the same text as the desktop shortcut's.
pub const DESCRIPTION: &str = crate::desktop_shortcut::DESCRIPTION;

/// Where under `HKEY_CURRENT_USER` the registration goes: `Software`, so the classes land in
/// `Software\Classes`, the current user's part of `HKEY_CLASSES_ROOT`. Tests use a scratch key.
const ROOT: &str = "Software";

/// Add `exe`, the app's executable, to the `.pdf` "Open with" list and to Default apps for the
/// current user (replacing an earlier registration, from this copy or another). Elsewhere than
/// Windows it is [`std::io::ErrorKind::Unsupported`].
pub fn register(exe: &Path) -> std::io::Result<()> {
    imp::run(Action::Register, checked(exe)?, ROOT)
}

/// Remove what [`register`] wrote. `exe` names the `Applications\<file name>` entry to remove.
/// Nothing registered is not an error. Elsewhere than Windows it is
/// [`std::io::ErrorKind::Unsupported`].
pub fn unregister(exe: &Path) -> std::io::Result<()> {
    imp::run(Action::Unregister, checked(exe)?, ROOT)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Register,
    Unregister,
}

/// The executable must be an absolute path with a file name: the registry stores it as is.
fn checked(exe: &Path) -> std::io::Result<&Path> {
    if !exe.is_absolute() || exe.file_name().is_none() {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "the program to register must be an absolute path to its executable"));
    }
    Ok(exe)
}

#[cfg(windows)]
mod imp {
    use super::Action;
    use std::os::windows::process::CommandExt as _;
    use std::path::{Path, PathBuf};

    /// `CREATE_NO_WINDOW`: PowerShell is a console program and the app is not, so without it a
    /// console window would flash up.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    /// Reads `AJS_OW_ACTION`, `AJS_OW_EXE`, `AJS_OW_ROOT`, `AJS_OW_PROGID`, `AJS_OW_NAME` and
    /// `AJS_OW_DESCRIPTION`. Every failure is an exception, so a non-zero exit, with its message on
    /// stderr. Unregistering deletes only the keys and values registering writes, and the AJS key
    /// above Capabilities only when nothing else is left in it.
    const SCRIPT: &str = r#"$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$hk = [Microsoft.Win32.Registry]::CurrentUser
$root = $env:AJS_OW_ROOT
$prog = $env:AJS_OW_PROGID
$name = $env:AJS_OW_NAME
$exe = $env:AJS_OW_EXE
$exeName = [IO.Path]::GetFileName($exe)
$classes = "$root\Classes"
$appKey = "$classes\Applications\$exeName"
$ajsKey = "$root\AJS\$name"
$cap = "$ajsKey\Capabilities"
function Set-Value([string]$path, [string]$valueName, [string]$value) {
    $k = $hk.CreateSubKey($path)
    if ($null -eq $k) { throw "cannot write HKEY_CURRENT_USER\$path" }
    try { $k.SetValue($valueName, $value, [Microsoft.Win32.RegistryValueKind]::String) } finally { $k.Dispose() }
}
function Remove-Value([string]$path, [string]$valueName) {
    $k = $hk.OpenSubKey($path, $true)
    if ($null -ne $k) { try { $k.DeleteValue($valueName, $false) } finally { $k.Dispose() } }
}
function Remove-IfEmpty([string]$path) {
    $k = $hk.OpenSubKey($path)
    if ($null -eq $k) { return }
    $empty = $k.SubKeyCount -eq 0 -and $k.ValueCount -eq 0
    $k.Dispose()
    if ($empty) { $hk.DeleteSubKey($path, $false) }
}
if ($env:AJS_OW_ACTION -eq 'register') {
    $open = '"' + $exe + '" "%1"'
    Set-Value "$classes\$prog" '' 'PDF Document'
    Set-Value "$classes\$prog\DefaultIcon" '' ($exe + ',-2')
    Set-Value "$classes\$prog\shell\open\command" '' $open
    Set-Value "$classes\.pdf\OpenWithProgids" $prog ''
    Set-Value $appKey 'FriendlyAppName' $name
    Set-Value "$appKey\SupportedTypes" '.pdf' ''
    Set-Value "$appKey\shell\open\command" '' $open
    Set-Value $cap 'ApplicationName' $name
    Set-Value $cap 'ApplicationDescription' $env:AJS_OW_DESCRIPTION
    Set-Value $cap 'ApplicationIcon' ($exe + ',0')
    Set-Value "$cap\FileAssociations" '.pdf' $prog
    Set-Value "$root\RegisteredApplications" $name $cap
} elseif ($env:AJS_OW_ACTION -eq 'unregister') {
    $hk.DeleteSubKeyTree("$classes\$prog", $false)
    Remove-Value "$classes\.pdf\OpenWithProgids" $prog
    $hk.DeleteSubKeyTree($appKey, $false)
    $hk.DeleteSubKeyTree($cap, $false)
    Remove-IfEmpty $ajsKey
    Remove-IfEmpty "$root\AJS"
    Remove-Value "$root\RegisteredApplications" $name
} else {
    throw "unknown action $($env:AJS_OW_ACTION)"
}
if ($root -eq 'Software') {
    # SHCNE_ASSOCCHANGED: Explorer re-reads the associations now instead of at the next sign-in.
    Add-Type -Namespace AjsOpenWith -Name Shell -MemberDefinition '[DllImport("shell32.dll")] public static extern void SHChangeNotify(int eventId, uint flags, IntPtr item1, IntPtr item2);'
    [AjsOpenWith.Shell]::SHChangeNotify(0x08000000, 0, [IntPtr]::Zero, [IntPtr]::Zero)
}
[Console]::Out.Write('ok')"#;

    pub(super) fn run(action: Action, exe: &Path, root: &str) -> std::io::Result<()> {
        let system = std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
        // By full path: a `powershell.exe` next to the app or earlier on the search path must not answer.
        let mut command = std::process::Command::new(system.join(r"System32\WindowsPowerShell\v1.0\powershell.exe"));
        command
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", SCRIPT])
            .env("AJS_OW_ACTION", if action == Action::Register { "register" } else { "unregister" })
            .env("AJS_OW_ROOT", root)
            .env("AJS_OW_PROGID", super::PROG_ID)
            .env("AJS_OW_NAME", super::APP_NAME)
            .env("AJS_OW_DESCRIPTION", super::DESCRIPTION)
            .env("AJS_OW_EXE", exe)
            .stdin(std::process::Stdio::null())
            .creation_flags(CREATE_NO_WINDOW);
        let out = command.output()?;
        if !out.status.success() || out.stdout.as_slice() != b"ok" {
            let err = String::from_utf8_lossy(&out.stderr);
            let first = err.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("PowerShell failed");
            return Err(std::io::Error::other(first.to_string()));
        }
        Ok(())
    }
}

#[cfg(not(windows))]
mod imp {
    use super::Action;
    use std::path::Path;

    pub(super) fn run(_action: Action, _exe: &Path, _root: &str) -> std::io::Result<()> {
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "\"Open with\" for PDF files is set up on Windows only"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_relative_program_is_refused() {
        for exe in ["AJSPdfEditor.exe", ""] {
            assert_eq!(register(Path::new(exe)).err().map(|e| e.kind()), Some(std::io::ErrorKind::InvalidInput), "{exe:?}");
            assert_eq!(unregister(Path::new(exe)).err().map(|e| e.kind()), Some(std::io::ErrorKind::InvalidInput), "{exe:?}");
        }
    }

    #[cfg(windows)]
    mod windows {
        use super::super::{Action, PROG_ID, imp};
        use std::path::Path;
        use std::process::Command;

        /// `reg query` on a key under the scratch root: its output, or `None` when the key is missing.
        fn query(root: &str, key: &str, value: Option<&str>) -> Option<String> {
            let mut c = Command::new("reg");
            c.args(["query", &format!(r"HKCU\{root}\{key}")]);
            match value {
                Some(v) => c.args(["/v", v]),
                None => c.arg("/ve"),
            };
            let out = c.output().ok()?;
            out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
        }

        /// Deletes the scratch root even when an assertion fails.
        struct Scratch(String);
        impl Drop for Scratch {
            fn drop(&mut self) {
                let _ = Command::new("reg").args(["delete", &format!(r"HKCU\{}", self.0), "/f"]).output();
            }
        }

        #[test]
        fn registers_under_the_root_and_removes_it_again() -> std::io::Result<()> {
            let scratch = Scratch(format!(r"Software\AJS-OpenWith-Test-{}", std::process::id()));
            let root = scratch.0.as_str();
            // Something else under the AJS key must survive unregistering.
            let status = Command::new("reg").args(["add", &format!(r"HKCU\{root}\AJS\Other"), "/f"]).output()?.status;
            assert!(status.success());
            let exe = Path::new(r"C:\Program Files\AJS 'x'\AJSPdfEditor.exe");

            imp::run(Action::Register, exe, root)?;
            let open = query(root, &format!(r"Classes\{PROG_ID}\shell\open\command"), None).unwrap_or_default();
            assert!(open.contains(r#""C:\Program Files\AJS 'x'\AJSPdfEditor.exe" "%1""#), "{open}");
            assert!(query(root, r"Classes\.pdf\OpenWithProgids", Some(PROG_ID)).is_some());
            assert!(query(root, r"Classes\Applications\AJSPdfEditor.exe\SupportedTypes", Some(".pdf")).is_some());
            let registered = query(root, "RegisteredApplications", Some("AJS PDF Editor")).unwrap_or_default();
            assert!(registered.contains(&format!(r"{root}\AJS\AJS PDF Editor\Capabilities")), "{registered}");
            // Again, from another copy: the same keys, now pointing at it.
            let other = Path::new(r"D:\Portable\AJSPdfEditor.exe");
            imp::run(Action::Register, other, root)?;
            let open = query(root, &format!(r"Classes\{PROG_ID}\shell\open\command"), None).unwrap_or_default();
            assert!(open.contains(r"D:\Portable\AJSPdfEditor.exe"), "{open}");

            imp::run(Action::Unregister, other, root)?;
            assert!(query(root, &format!(r"Classes\{PROG_ID}"), None).is_none());
            assert!(query(root, r"Classes\.pdf\OpenWithProgids", Some(PROG_ID)).is_none());
            assert!(query(root, r"Classes\Applications\AJSPdfEditor.exe", None).is_none());
            assert!(query(root, r"AJS\AJS PDF Editor", None).is_none());
            assert!(query(root, r"AJS\Other", None).is_some());
            assert!(query(root, "RegisteredApplications", Some("AJS PDF Editor")).is_none());
            // Removing what is no longer there is not an error.
            imp::run(Action::Unregister, other, root)?;
            Ok(())
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn elsewhere_it_is_unsupported() {
        let err = register(Path::new("/usr/bin/pdfcraft")).err().map(|e| e.kind());
        assert_eq!(err, Some(std::io::ErrorKind::Unsupported));
    }
}
