# ForgeBox Linux packages

ForgeBox releases provide four Linux distribution formats:

| Format | Intended systems | Install |
|---|---|---|
| RPM | Fedora and other RPM distributions | `sudo dnf install ./forgebox-*.rpm` |
| DEB | Debian and Ubuntu | `sudo apt install ./forgebox_*.deb` |
| Pacman | Arch Linux | `sudo pacman -U ./forgebox-*.pkg.tar.zst` |
| AppImage | Compatible x86_64 Linux distributions | `chmod +x ./forgebox_*.AppImage && ./forgebox_*.AppImage` |

The Arch release includes a native Pacman package, plus its `PKGBUILD` and `.SRCINFO` for rebuilding. Package names are kept consistent as `forgebox`.

The DEB and Pacman packages declare the native libraries and common conversion engines used by ForgeBox. The exact dependency names follow each distribution's package repositories.

## AppImage limitations

AppImage bundles ForgeBox, its icon and desktop entry, and the shared libraries detected by the packager. It does **not** bundle every command-line conversion engine. Install the engines required by the features you use:

- ImageMagick for image conversion and image-to-PDF.
- Poppler utilities for PDF merge, split and rendering.
- Ghostscript and qpdf for PDF compression and page operations.
- LibreOffice for document and spreadsheet conversion.
- Tesseract and language data for OCR.
- ExifTool for EXIF extraction.
- FFmpeg for video, audio and GIF features (optional).
- `pdftk` for PDF watermarking.
- `rembg` and its Python model environment for background removal. Fedora RPM users can run `/usr/share/doc/forgebox/install-rembg-fedora.sh` to set up the optional CPU backend in an isolated Python 3.13 environment.

AppImage still relies on the host's Linux kernel and glibc baseline. It is not a replacement for a fully self-contained virtual machine.

## RPM signing

Tagged GitHub releases are configured to publish **signed RPMs**. Manual workflow runs and branch builds produce unsigned test RPMs.

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

The release workflow exports the corresponding public key as `forgebox-rpm-signing-key.asc` and verifies the RPM signature before publishing. Users can import the public key and then verify/install the package:

```bash
sudo rpm --import forgebox-rpm-signing-key.asc
rpmkeys --checksig --verbose ./forgebox-*.rpm
sudo dnf install ./forgebox-*.rpm
```

The public-key fingerprint is also published in `forgebox-rpm-signing-key-id.txt`. Users should obtain the public key from the official ForgeBox release and compare the fingerprint before trusting it.

## Release workflow

The Linux workflow runs tests, builds RPM, DEB and AppImage outputs, builds a native Arch package with `makepkg`, and publishes SHA-256 checksums with tagged GitHub releases.

The current workflow does not publish to a signed APT or Pacman repository. DEB and Arch users install the downloaded release artifacts directly. AUR/Flathub-style repository publication can be added separately after the first package builds are verified.
