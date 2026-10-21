param(
  [switch]$SkipOffice,
  [switch]$SkipVideo
)
$ErrorActionPreference = 'Continue'
$winget = Get-Command winget -ErrorAction SilentlyContinue
if (-not $winget) {
  Write-Error 'WinGet was not found. Install Microsoft App Installer, then run this script again.'
  exit 2
}

$packages = @(
  @{ Id = 'ImageMagick.Q16'; Name = 'ImageMagick'; Optional = $false },
  @{ Id = 'oschwartz10612.Poppler'; Name = 'Poppler PDF tools'; Optional = $false },
  @{ Id = 'ArtifexSoftware.GhostScript'; Name = 'Ghostscript'; Optional = $false },
  @{ Id = 'QPDF.QPDF'; Name = 'qpdf'; Optional = $false },
  @{ Id = 'UB-Mannheim.TesseractOCR'; Name = 'Tesseract OCR'; Optional = $false },
  @{ Id = 'OliverBetz.ExifTool'; Name = 'ExifTool'; Optional = $false },
  @{ Id = 'TheDocumentFoundation.LibreOffice'; Name = 'LibreOffice'; Optional = $true },
  @{ Id = 'Gyan.FFmpeg'; Name = 'FFmpeg'; Optional = $true }
)
$failed = [System.Collections.Generic.List[string]]::new()
foreach ($package in $packages) {
  if ($SkipOffice -and $package.Id -eq 'TheDocumentFoundation.LibreOffice') { continue }
  if ($SkipVideo -and $package.Id -eq 'Gyan.FFmpeg') { continue }
  Write-Host "`n==> Installing $($package.Name) [$($package.Id)]" -ForegroundColor Cyan
  & $winget.Source install --exact --id $package.Id --silent --accept-package-agreements --accept-source-agreements --disable-interactivity
  if ($LASTEXITCODE -ne 0) {
    $failed.Add("$($package.Name) ($($package.Id), exit $LASTEXITCODE)")
    Write-Warning "Installation failed for $($package.Name); continuing with the other packages."
  }
}
Write-Host "`nFinished. Close and reopen ForgeBox (and, if needed, sign out/in) so it receives the updated PATH."
if ($failed.Count -gt 0) {
  Write-Warning 'Some packages failed to install:'
  $failed | ForEach-Object { Write-Warning "  $_" }
  Write-Warning 'Run winget install manually for failed packages, then ensure their command-line folders are in PATH.'
}
Write-Host 'Notes:'
Write-Host '  - PDF watermarking still needs a pdftk executable available in PATH.'
Write-Host '  - Background removal needs rembg in a Python environment.'
Write-Host '  - H.264/VP9 support depends on the FFmpeg build and available codecs.'
