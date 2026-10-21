$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$commit = '798bb7b6afa343ed34d19040db48248ce14d2108'
$repo = 'https://github.com/MixaDoDs/photocraft.git'
$branch = 'wayland-dnd-main'
$dest = Join-Path $root 'patches\winit'
$requiredFile = Join-Path $dest 'src\platform_impl\linux\wayland\seat\dnd.rs'
if ((Test-Path (Join-Path $dest 'Cargo.toml')) -and (Test-Path $requiredFile)) {
  $manifest = Get-Content (Join-Path $dest 'Cargo.toml') -Raw
  if ($manifest -match 'version\s*=\s*"0\.30\.13"' -and (Select-String -Path $requiredFile -Pattern 'DataDeviceHandler' -Quiet)) {
    Write-Host 'Pinned winit source is already present.'
    exit 0
  }
}
if (Test-Path $dest) { throw "Found an unexpected folder at $dest. Move it aside before retrying." }
if (-not (Get-Command git -ErrorAction SilentlyContinue)) { throw 'Git is required to prepare the pinned winit patch.' }
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("forgebox-winit-" + [guid]::NewGuid().ToString('N'))
try {
  git clone --quiet --depth=1 --filter=blob:none --no-checkout --branch $branch $repo $tmp
  if ($LASTEXITCODE -ne 0) { throw 'Could not clone the pinned winit source.' }
  git -C $tmp cat-file -e "$commit^{commit}" 2>$null
  if ($LASTEXITCODE -ne 0) {
    git -C $tmp fetch --quiet --depth=1 --filter=blob:none origin $commit
    if ($LASTEXITCODE -ne 0) { throw 'Could not fetch the pinned winit commit.' }
  }
  git -C $tmp sparse-checkout init --cone
  if ($LASTEXITCODE -ne 0) { throw 'Could not initialize sparse checkout.' }
  git -C $tmp sparse-checkout set patches/winit
  if ($LASTEXITCODE -ne 0) { throw 'Could not select patches/winit.' }
  git -C $tmp checkout --quiet --detach $commit
  if ($LASTEXITCODE -ne 0) { throw 'Could not check out the pinned winit commit.' }
  $actual = (git -C $tmp rev-parse HEAD).Trim()
  if ($actual -ne $commit) { throw "Unexpected winit revision $actual; expected $commit." }
  $sourceRoot = Join-Path $tmp 'patches\winit'
  $sourceCandidate = Join-Path $sourceRoot 'src\platform_impl\linux\wayland\seat\dnd.rs'
  $sourceManifest = Join-Path $sourceRoot 'Cargo.toml'
  if (-not (Test-Path $sourceCandidate) -or -not (Test-Path $sourceManifest)) {
    throw 'Pinned source has no expected Wayland DnD implementation.'
  }
  $sourceText = Get-Content $sourceManifest -Raw
  if ($sourceText -notmatch 'version\s*=\s*"0\.30\.13"' -or -not (Select-String -Path $sourceCandidate -Pattern 'DataDeviceHandler' -Quiet)) {
    throw 'Pinned winit checkout does not match the expected 0.30.13 patch.'
  }
  New-Item -ItemType Directory -Force (Split-Path $dest) | Out-Null
  Copy-Item (Join-Path $tmp 'patches\winit') $dest -Recurse
  Write-Host "Prepared pinned winit source at $dest"
} finally {
  Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
}
