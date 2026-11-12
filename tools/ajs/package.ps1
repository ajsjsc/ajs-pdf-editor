<#
  AJS fork: build and package AJS PDF Editor for Windows x64 (portable zip).

    dist-ajs\AJSPdfEditor-<version>-windows-x64-portable.zip
      AJSPdfEditor.exe, AJSPdfEditor-cli.exe, models\, portable.txt, licences and notices
    dist-ajs\AJSPdfEditor-<version>-windows-x64-portable.zip.sha256

  Same build as the official release (packaging/windows/package.ps1): static CRT, Windows resources
  required, craft-fonts embedded (CRAFT_FONTS_DIR). The executables are renamed after the build.
  Translations: since PdfCraft 0.21 they live in storytold/craft-i18n (at the commit in
  craft-i18n.rev); `rebrand.pl --catalogs` renames the product in them and adds AJS's own rows
  (tools\ajs\i18n) into target\ajs-i18n, which the build embeds through CRAFT_I18N_DIR.

  Usage (Windows PowerShell 5.1 or pwsh, from anywhere):
    tools\ajs\package.ps1 -CraftFonts C:\Dev\craft-fonts -CraftI18n C:\Dev\craft-i18n
    tools\ajs\package.ps1 -CraftFonts C:\Dev\craft-fonts -CraftI18n C:\Dev\craft-i18n -SkipBuild -ModelsDir C:\path\to\models
#>
param(
  [Parameter(Mandatory = $true)] [string] $CraftFonts,
  # A storytold/craft-i18n checkout at the commit in craft-i18n.rev.
  [Parameter(Mandatory = $true)] [string] $CraftI18n,
  [switch] $SkipBuild,
  # Reuse OCR models from a folder (e.g. models\ of an official portable zip); otherwise `cargo xtask models` downloads them.
  [string] $ModelsDir = '',
  [string] $Label = 'ajs'
)
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Target = 'x86_64-pc-windows-msvc'
Set-Location $Root

function Invoke-Native([string] $what, [scriptblock] $cmd) {
  # Windows PowerShell 5.1 turns a native program's stderr (cargo's progress and warnings) into
  # errors under 'Stop': judge the run by its exit code only.
  $saved = $ErrorActionPreference
  $ErrorActionPreference = 'Continue'
  try { & $cmd 2>&1 | ForEach-Object { "$_" } } finally { $ErrorActionPreference = $saved }
  if ($LASTEXITCODE -ne 0) { throw "$what failed (exit $LASTEXITCODE)" }
}

$Version = (Select-String -Path (Join-Path $Root 'Cargo.toml') -Pattern '^version = "([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
if (-not $Version) { throw 'version not found in Cargo.toml' }
Write-Output "AJS PDF Editor $Version ($Label) for Windows x64"

# ---- checks: the AJS changes are still in place (after an upstream merge) ----------------------
$main = Get-Content -Raw (Join-Path $Root 'apps\pdfcraft\src\main.rs')
if ($main -notmatch 'set_startup_update_check\(false\)') { throw 'the startup update check is not disabled (apps/pdfcraft/src/main.rs)' }
if (Test-Path (Join-Path $Root 'docs\brand')) { throw 'docs\brand (ArtCraft trademarks) is back: delete it' }
$brand = git -C $Root grep -n -E 'artcraft_(mark|logo)\(' -- crates apps
if ($brand) { throw "ArtCraft marks still drawn:`n$brand" }

# ---- build -------------------------------------------------------------------------------------
if (-not $SkipBuild) {
  [Environment]::SetEnvironmentVariable('CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS', '-C target-feature=+crt-static')
  $env:PDFCRAFT_REQUIRE_WINRES = '1'
  $env:CRAFT_FONTS_DIR = (Resolve-Path $CraftFonts).Path
  $env:PDFCRAFT_BUILD_LABEL = $Label
  # Translations: craft-i18n's catalogs with the product renamed, plus AJS's own rows.
  $i18nSrc = (Resolve-Path $CraftI18n).Path
  $want = (Get-Content -Raw (Join-Path $Root 'craft-i18n.rev')).Trim()
  $have = "$(git -C $i18nSrc rev-parse HEAD)".Trim()
  if ($have -ne $want) { throw "craft-i18n is at $have but craft-i18n.rev wants $want (git -C $i18nSrc checkout $want)" }
  $perl = (Get-Command perl -ErrorAction SilentlyContinue).Source
  if (-not $perl) { $perl = Join-Path (Split-Path (Split-Path (Get-Command git).Source)) 'usr\bin\perl.exe' }
  $i18nOut = Join-Path $Root 'target\ajs-i18n'
  if (Test-Path $i18nOut) { Remove-Item -Recurse -Force $i18nOut }
  Invoke-Native 'rebrand.pl --catalogs' { & $perl tools/ajs/rebrand.pl --catalogs $i18nSrc $i18nOut }
  $env:CRAFT_I18N_DIR = $i18nOut
  $env:CRAFT_I18N_REQUIRED = '1'
  Invoke-Native 'cargo build' { cargo build --release --locked -p pdfcraft -p pdfcraft-cli --target $Target }
}
$Bin = Join-Path $Root "target\$Target\release"

# ---- stage -------------------------------------------------------------------------------------
$Dist = Join-Path $Root 'dist-ajs'
$Name = "AJSPdfEditor-$Version-windows-x64-portable"
$Stage = Join-Path $Dist $Name
if (Test-Path $Stage) { Remove-Item -Recurse -Force $Stage }
New-Item -ItemType Directory -Force -Path $Stage | Out-Null

Copy-Item (Join-Path $Bin 'pdfcraft.exe') (Join-Path $Stage 'AJSPdfEditor.exe')
Copy-Item (Join-Path $Bin 'pdfcraft-cli.exe') (Join-Path $Stage 'AJSPdfEditor-cli.exe')

foreach ($f in 'LICENSE-MIT', 'LICENSE-APACHE', 'NOTICE', 'AJS-FORK.md') { Copy-Item (Join-Path $Root $f) $Stage }
Copy-Item (Join-Path $Root 'assets\app-icon\LICENSE-document.txt') $Stage

# Fonts embedded from craft-fonts: their OFL notices, as the official package ships them.
. (Join-Path $Root 'packaging\windows\font-notices.ps1')
foreach ($n in @(Get-CraftFontNotices (Resolve-Path $CraftFonts).Path $false)) {
  # fonts\<family>\OFL.txt -> OFL-<family>.txt, as the official portable zip names them.
  Copy-Item $n.FullName (Join-Path $Stage ("OFL-{0}.txt" -f $n.Directory.Name))
}

# OCR models (CC-BY-SA-4.0, with their ATTRIBUTION.txt and licence files).
$Models = Join-Path $Stage 'models'
if ($ModelsDir) {
  Copy-Item -Recurse (Resolve-Path $ModelsDir).Path $Models
} else {
  New-Item -ItemType Directory -Force -Path $Models | Out-Null
  Invoke-Native 'cargo xtask models' { cargo xtask models $Models }
  Remove-Item -Force (Join-Path $Models '*.part') -ErrorAction SilentlyContinue
}
if (-not (Test-Path (Join-Path $Models 'ATTRIBUTION.txt'))) { throw "no OCR models in $Models" }

# Portable mode: settings, logs and recovery files stay in PdfCraftData beside the executable.
@'
AJS PDF Editor portable mode

While this file sits next to AJSPdfEditor.exe, AJS PDF Editor keeps its settings, log files,
crash-recovery autosaves and new digital IDs in the PdfCraftData folder beside it.
Delete this file to use the per-user folders instead.
'@ | Set-Content -Encoding UTF8 (Join-Path $Stage 'portable.txt')

# ---- zip + checksum ----------------------------------------------------------------------------
$Zip = Join-Path $Dist "$Name.zip"
Remove-Item -Force $Zip -ErrorAction SilentlyContinue
Compress-Archive -Path $Stage -DestinationPath $Zip
$hash = (Get-FileHash $Zip -Algorithm SHA256).Hash.ToLowerInvariant()
"$hash  $Name.zip" | Set-Content -Encoding ASCII "$Zip.sha256"

& (Join-Path $Stage 'AJSPdfEditor-cli.exe') --version
Get-Item $Zip | Format-Table Name, @{ n = 'MB'; e = { [math]::Round($_.Length / 1MB, 1) } }
Write-Output "SHA-256 $hash"
