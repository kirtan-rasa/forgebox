$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$cargo = Get-Content (Join-Path $root 'Cargo.toml') -Raw
$match = [regex]::Match($cargo, '(?m)^version\s*=\s*"([^"]+)"')
if (-not $match.Success) { throw 'Could not read package version from Cargo.toml' }
$version = $match.Groups[1].Value
$numeric = ($version -split '-', 2)[0]
$parts = @($numeric.Split('.'))
while ($parts.Count -lt 4) { $parts += '0' }
$numericVersion = ($parts | Select-Object -First 4) -join '.'
$templatePath = Join-Path $PSScriptRoot 'forgebox.iss.in'
$generatedPath = Join-Path $PSScriptRoot 'forgebox.generated.iss'
$template = Get-Content $templatePath -Raw
$template = $template.Replace('@APP_VERSION@', $version).Replace('@APP_NUMERIC_VERSION@', $numericVersion)
[System.IO.File]::WriteAllText($generatedPath, $template, [System.Text.UTF8Encoding]::new($false))
$possibleCompiler = @(
  (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'),
  (Join-Path $env:ProgramFiles 'Inno Setup 6\ISCC.exe')
) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
if (-not $possibleCompiler) { throw 'Inno Setup compiler (ISCC.exe) was not found. Install it with: choco install innosetup -y' }
New-Item -ItemType Directory -Force (Join-Path $root 'dist') | Out-Null
try {
  & $possibleCompiler $generatedPath
  if ($LASTEXITCODE -ne 0) { throw "Inno Setup failed with exit code $LASTEXITCODE" }
} finally {
  Remove-Item $generatedPath -Force -ErrorAction SilentlyContinue
}
$stage = Join-Path $root 'dist\portable\ForgeBox'
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item (Join-Path $root 'target\release\forgebox.exe') (Join-Path $stage 'forgebox.exe') -Force
Copy-Item (Join-Path $root 'assets\forgebox.ico') (Join-Path $stage 'forgebox.ico') -Force
Copy-Item (Join-Path $root 'docs\WINDOWS-RUNTIME-DEPENDENCIES.md') (Join-Path $stage 'README-Windows.txt') -Force
Copy-Item (Join-Path $root 'LICENSE') (Join-Path $stage 'LICENSE.txt') -Force
$zipPath = Join-Path $root 'dist\ForgeBox-portable-x64.zip'
Remove-Item $zipPath -Force -ErrorAction SilentlyContinue
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zipPath -CompressionLevel Optimal
Write-Host "Created installer and portable package in $root\dist"
