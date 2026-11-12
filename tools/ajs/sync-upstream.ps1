<#
  AJS fork: bring a new PdfCraft release into AJS PDF Editor. Run it only when the admin decides to
  ship a new version; nothing in AJS PDF Editor checks for upstream releases by itself.

    tools\ajs\sync-upstream.ps1 -Tag v0.19.0

  1. fetches the tag from the upstream remote (storytold/pdfcraft),
  2. merges it into the current branch (ajs/main) — stops on conflicts, listing the files,
  3. re-applies the product rename (tools/ajs/rebrand.pl) and deletes docs/brand again,
  4. checks the AJS changes are still in place.
  Then build and test with tools\ajs\package.ps1, and commit.
#>
param([Parameter(Mandatory = $true)] [string] $Tag)
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Set-Location $Root

if (git status --porcelain) { throw 'the working tree has uncommitted changes: commit or discard them first' }
if (-not (git remote | Select-String -SimpleMatch 'upstream')) { git remote add upstream https://github.com/storytold/pdfcraft.git }
git fetch upstream "refs/tags/${Tag}:refs/tags/$Tag"
if ($LASTEXITCODE -ne 0) { throw "tag $Tag not found upstream" }

git merge --no-ff --no-edit -m "chore(ajs): đồng bộ PdfCraft $Tag" $Tag
if ($LASTEXITCODE -ne 0) {
  Write-Warning 'merge conflicts: resolve them (keep the AJS changes listed in AJS-FORK.md), then run this script again with -Tag to re-check:'
  git diff --name-only --diff-filter=U
  exit 1
}

# Upstream may re-add brand files or new "PdfCraft" strings.
if (Test-Path 'docs\brand') { git rm -r -q docs/brand }
perl tools/ajs/rebrand.pl
if ($LASTEXITCODE -ne 0) { throw 'rebrand.pl failed' }

$problems = @()
if ((Get-Content -Raw 'apps\pdfcraft\src\main.rs') -notmatch 'set_startup_update_check\(false\)') { $problems += 'apps/pdfcraft/src/main.rs: set_startup_update_check(false) missing' }
if (git grep -n -E 'artcraft_(mark|logo)\(' -- crates apps) { $problems += 'ArtCraft marks are drawn again (artcraft_mark / artcraft_logo)' }
if (git grep -n -E '(getartcraft\.com|discord\.gg/artcraft)' -- crates/engine/src crates/ui-egui/src apps) { $problems += 'ArtCraft links are back in the app' }
if ($problems) { $problems | ForEach-Object { Write-Warning $_ }; exit 1 }

if (git status --porcelain) {
  git add -A
  git commit -q -m "chore(ajs): áp dụng lại đổi tên AJS PDF Editor sau khi đồng bộ $Tag"
}
Write-Output "Merged $Tag. Next: tools\ajs\package.ps1 -CraftFonts <craft-fonts at the revision in .github/workflows/release.yml> -CraftI18n <craft-i18n at the commit in craft-i18n.rev>"
