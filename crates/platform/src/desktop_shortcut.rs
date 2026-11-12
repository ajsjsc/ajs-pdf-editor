//! AJS fork: put an "AJS PDF Editor" shortcut on the user's desktop (Help ▸ Create desktop shortcut).
//!
//! Windows only. The shortcut is `<Desktop>\AJS PDF Editor.lnk`, the same file the AJS AutoCAD
//! plug-in (D2P) writes when it installs or updates the editor, so the two never leave duplicates:
//! whichever runs last points it at its executable. The `.lnk` is written by Windows' own shell
//! object (`IShellLinkW`) through the system PowerShell, so this needs no `unsafe` COM code. The
//! paths reach the script through environment variables, never through the script text, so no
//! file name can change what the script does.

use std::path::{Path, PathBuf};

/// The shortcut's file name on the desktop.
pub const FILE_NAME: &str = "AJS PDF Editor.lnk";

/// The shortcut's comment (its tooltip), the same text the AJS plug-in writes.
pub const DESCRIPTION: &str = "AJS PDF Editor – xem và chỉnh sửa PDF";

/// Create (or replace) the desktop shortcut to `target`, the app's executable, and return the
/// shortcut's path. `desktop` overrides the folder (tests); `None` is the user's desktop as Windows
/// reports it, which may be redirected (OneDrive). Elsewhere than Windows it is
/// [`std::io::ErrorKind::Unsupported`].
pub fn create(target: &Path, desktop: Option<&Path>) -> std::io::Result<PathBuf> {
    if !target.is_absolute() {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "the shortcut's target must be an absolute path"));
    }
    imp::create(target, desktop)
}

#[cfg(windows)]
mod imp {
    use std::os::windows::process::CommandExt as _;
    use std::path::{Path, PathBuf};

    /// `CREATE_NO_WINDOW`: PowerShell is a console program and the app is not, so without it a
    /// console window would flash up.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    /// Reads `AJS_SHORTCUT_TARGET` (and `AJS_SHORTCUT_DIR`, when set) and prints the shortcut's path
    /// as UTF-8. Every failure is an exception, so a non-zero exit, with its message on stderr.
    ///
    /// The shell's own `IShellLinkW` (the Unicode one) writes the `.lnk`. `WScript.Shell` is not used:
    /// its shortcut object goes through the ANSI code page, so it can't save to a desktop or point at
    /// a program whose path has letters outside it (Vietnamese ones in a user name, "Bàn làm việc").
    const SCRIPT: &str = r#"$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Runtime.InteropServices.ComTypes;
using System.Text;
[ComImport, Guid("00021401-0000-0000-C000-000000000046")]
public class AjsShellLink {}
[ComImport, InterfaceType(ComInterfaceType.InterfaceIsIUnknown), Guid("000214F9-0000-0000-C000-000000000046")]
public interface IAjsShellLinkW {
    void GetPath([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder file, int cch, IntPtr data, int flags);
    void GetIDList(out IntPtr pidl);
    void SetIDList(IntPtr pidl);
    void GetDescription([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder name, int cch);
    void SetDescription([MarshalAs(UnmanagedType.LPWStr)] string name);
    void GetWorkingDirectory([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder dir, int cch);
    void SetWorkingDirectory([MarshalAs(UnmanagedType.LPWStr)] string dir);
    void GetArguments([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder args, int cch);
    void SetArguments([MarshalAs(UnmanagedType.LPWStr)] string args);
    void GetHotkey(out short hotkey);
    void SetHotkey(short hotkey);
    void GetShowCmd(out int show);
    void SetShowCmd(int show);
    void GetIconLocation([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder path, int cch, out int index);
    void SetIconLocation([MarshalAs(UnmanagedType.LPWStr)] string path, int index);
    void SetRelativePath([MarshalAs(UnmanagedType.LPWStr)] string path, int reserved);
    void Resolve(IntPtr hwnd, int flags);
    void SetPath([MarshalAs(UnmanagedType.LPWStr)] string file);
}
public static class AjsShortcut {
    public static void Save(string lnk, string target, string dir, string description) {
        var link = (IAjsShellLinkW)new AjsShellLink();
        link.SetPath(target);
        link.SetWorkingDirectory(dir);
        link.SetIconLocation(target, 0);
        link.SetDescription(description);
        ((IPersistFile)link).Save(lnk, true);
    }
}
'@
$target = $env:AJS_SHORTCUT_TARGET
$dir = $env:AJS_SHORTCUT_DIR
if (-not $dir) { $dir = [Environment]::GetFolderPath('Desktop') }
if (-not $dir) { throw 'Windows reports no desktop folder' }
if (-not [IO.Directory]::Exists($dir)) { throw "the folder $dir does not exist" }
$lnk = [IO.Path]::Combine($dir, $env:AJS_SHORTCUT_NAME)
[AjsShortcut]::Save($lnk, $target, [IO.Path]::GetDirectoryName($target), $env:AJS_SHORTCUT_DESCRIPTION)
[Console]::Out.Write($lnk)"#;

    pub(super) fn create(target: &Path, desktop: Option<&Path>) -> std::io::Result<PathBuf> {
        let root = std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
        // By full path: a `powershell.exe` next to the app or earlier on the search path must not answer.
        let mut command = std::process::Command::new(root.join(r"System32\WindowsPowerShell\v1.0\powershell.exe"));
        command
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", SCRIPT])
            .env("AJS_SHORTCUT_TARGET", target)
            .env("AJS_SHORTCUT_NAME", super::FILE_NAME)
            .env("AJS_SHORTCUT_DESCRIPTION", super::DESCRIPTION)
            .env_remove("AJS_SHORTCUT_DIR")
            .stdin(std::process::Stdio::null())
            .creation_flags(CREATE_NO_WINDOW);
        if let Some(dir) = desktop {
            command.env("AJS_SHORTCUT_DIR", dir);
        }
        let out = command.output()?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            let first = err.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("PowerShell failed");
            return Err(std::io::Error::other(first.to_string()));
        }
        let path = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
        if path.as_os_str().is_empty() || !path.is_file() {
            return Err(std::io::Error::other("the shortcut was not written"));
        }
        Ok(path)
    }
}

#[cfg(not(windows))]
mod imp {
    use std::path::{Path, PathBuf};

    pub(super) fn create(_target: &Path, _desktop: Option<&Path>) -> std::io::Result<PathBuf> {
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "desktop shortcuts are made on Windows only"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_relative_target_is_refused() {
        let err = create(Path::new("AJSPdfEditor.exe"), None).err().map(|e| e.kind());
        assert_eq!(err, Some(std::io::ErrorKind::InvalidInput));
    }

    #[cfg(windows)]
    #[test]
    fn writes_the_shortcut_into_the_folder_and_replaces_it() -> std::io::Result<()> {
        // A folder name with Vietnamese letters and a quote: the paths travel by environment
        // variable, so neither breaks the script.
        let dir = std::env::temp_dir().join(format!("ajs-shortcut-test-{}-Bàn làm việc 'x'", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        let target = std::env::current_exe()?;
        let first = create(&target, Some(&dir))?;
        assert_eq!(first, dir.join(FILE_NAME));
        assert!(std::fs::metadata(&first)?.len() > 0);
        // Again: the same file, replaced, not a second shortcut.
        let second = create(&target, Some(&dir))?;
        assert_eq!(second, first);
        assert_eq!(std::fs::read_dir(&dir)?.count(), 1);
        std::fs::remove_dir_all(&dir)?;
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn a_missing_folder_is_an_error_not_a_panic() {
        let dir = std::env::temp_dir().join(format!("ajs-shortcut-missing-{}", std::process::id())).join("nope");
        let target = std::env::current_exe().unwrap_or_else(|_| PathBuf::from(r"C:\Windows\notepad.exe"));
        assert!(create(&target, Some(&dir)).is_err());
    }

    #[cfg(not(windows))]
    #[test]
    fn elsewhere_it_is_unsupported() {
        let err = create(Path::new("/usr/bin/pdfcraft"), None).err().map(|e| e.kind());
        assert_eq!(err, Some(std::io::ErrorKind::Unsupported));
    }
}
