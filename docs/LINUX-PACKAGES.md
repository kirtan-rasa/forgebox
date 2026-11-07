# ForgeBox Linux packages

ForgeBox releases provide four Linux distribution formats:

| Format | Intended systems | Install |
|---|---|---|
| RPM | Fedora and other RPM distributions | `sudo dnf install ./forgebox-*.rpm` |
| DEB | Debian and Ubuntu | `sudo apt install ./forgebox_*.deb` |
| Pacman | Arch Linux | `sudo pacman -U ./forgebox-*.pkg.tar.zst` |
| AppImage | Compatible x86_64 Linux distributions | `chmod +x ./forgebox_*.AppImage && ./forgebox_*.AppImage` |

The release includes the installable Arch Pacman package. Its `PKGBUILD` and `.SRCINFO` are used and validated inside CI, but are not attached to public releases.

RPM, DEB, and Pacman packages declare the native libraries and common conversion engines used by ForgeBox. Install them through the distribution's package manager so it can resolve dependencies. LibreOffice and FFmpeg are required runtime dependencies, alongside the image, PDF, OCR, and metadata utilities. The RPM and DEB packages require Tesseract and the English language data used by default OCR. Arch packages include `tesseract`, `tesseract-data-eng`, and `pdftk` for PDF watermarking. Russian OCR data is optional and can be installed from the post-install prompt.

On Fedora, the full FFmpeg build must be available from RPM Fusion Free because ForgeBox uses the `libx264` encoder for MP4/MOV/MKV output. Fedora's `ffmpeg-free` package does not include `libx264`; the quick installer enables RPM Fusion Free automatically. For manual RPM installation, see [Fedora runtime dependencies](FEDORA-RUNTIME-DEPENDENCIES.md).

The AppImage is different: it does not install host conversion engines or language data. Install the utilities required by the features you use.

## AppImage limitations

AppImage bundles ForgeBox, its icon and desktop entry, and the shared libraries detected by the packager. It does **not** bundle every command-line conversion engine. Install the engines required by the features you use:

- ImageMagick for image conversion and image normalization; img2pdf for assembling image pages into PDFs.
- Poppler utilities for PDF merge, split and rendering.
- Ghostscript and qpdf for PDF compression and page operations.
- LibreOffice for document and spreadsheet conversion (required for the full feature set).
- Tesseract and language data for OCR.
- ExifTool for EXIF extraction.
- FFmpeg for video, audio and GIF features (required for the media tools).
- `pdftk` for PDF watermarking.
- `rembg` and its Python model environment for background removal. Fedora RPM users can run `/usr/share/doc/forgebox/install-rembg-fedora.sh` to set up the optional CPU backend in an isolated Python 3.13 environment.

AppImage still relies on the host's Linux kernel and glibc baseline. It is not a replacement for a fully self-contained virtual machine.

## RPM signing

Tagged GitHub releases publish **signed RPMs**. Manual workflow runs and branch builds produce unsigned test RPMs.

The repository needs one GitHub Actions secret:

- Name: `RPM_SIGNING_PRIVATE_KEY`
- Value: the ASCII-armored private key for a dedicated ForgeBox RPM signing key.

Create a dedicated key on a trusted computer. Do not use a personal email-signing key and never add the private key to Git, an issue, a pull request, or a release artifact. The CI key is intentionally dedicated to RPM signing and should be protected and rotated independently.

Example key generation:

```bash
gpg --full-generate-key
```

Choose an RSA signing key (3072 bits or stronger), give it a clear ForgeBox release identity, and leave its passphrase empty so the isolated CI runner can sign without an interactive prompt. This key is for package signing only.

Export the secret key to a local file:

```bash
FINGERPRINT="$(gpg --with-colons --list-secret-keys "ForgeBox RPM Signing" | awk -F: '$1 == "fpr" { print $10; exit }')"
test -n "$FINGERPRINT"
gpg --armor --export-secret-keys "$FINGERPRINT" > forgebox-rpm-signing-private.asc
chmod 600 forgebox-rpm-signing-private.asc
```

Open GitHub → **Settings → Secrets and variables → Actions → New repository secret**. Use the name `RPM_SIGNING_PRIVATE_KEY` and paste the complete contents of that file as its value. Keep a secure offline backup of the private key. Do not commit the exported file.

The release workflow exports the corresponding public key as `forgebox-rpm-signing-key.asc` and verifies the RPM signature before publishing. The public key is included in each tagged release. Users can inspect its fingerprint, import the key, and then verify/install the package:

```bash
gpg --show-keys --with-fingerprint forgebox-rpm-signing-key.asc
sudo rpm --import forgebox-rpm-signing-key.asc
rpmkeys --checksig --verbose ./forgebox-*.rpm
sudo dnf install ./forgebox-*.rpm
```

The private key is never published. The small text file containing only the key ID is not distributed separately; the fingerprint can be read directly from the public key.

## Release workflow

The Linux workflow runs tests, builds RPM, DEB, AppImage, and a native Arch package, then publishes only the four installable packages and the public RPM signing key. Intermediate outputs such as the debug Arch package, `PKGBUILD`, `.SRCINFO`, and temporary source archives remain in the CI artifacts rather than the public release.

GitHub exposes a SHA-256 digest for each release asset. The one-command installer reads that digest from the release API and verifies the downloaded package before installation.

The current workflow does not publish to a signed APT or Pacman repository. DEB and Arch users install the downloaded release artifacts directly. AUR/Flathub-style repository publication can be added separately after the first package builds are verified.
