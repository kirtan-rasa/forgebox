# ForgeBox Linux packages

ForgeBox releases provide four Linux distribution formats:

| Format | Intended systems | Install |
|---|---|---|
| RPM | Fedora and other RPM distributions | `sudo dnf install ./forgebox-*.rpm` |
| DEB | Debian and Ubuntu | `sudo apt install ./forgebox-*.deb` |
| Pacman | Arch Linux | Extract the generated Pacman source archive and run `makepkg -si` in the directory containing `PKGBUILD` |
| AppImage | Compatible x86_64 Linux distributions | `chmod +x ForgeBox-*.AppImage && ./ForgeBox-*.AppImage` |

The DEB and Pacman packages declare the native libraries and common conversion engines used by ForgeBox. The exact dependency names follow each distribution's package repositories.

## AppImage limitations

AppImage bundles ForgeBox and the shared libraries detected by the packager. It does **not** bundle every command-line conversion engine. Install the engines required by the features you use:

- ImageMagick for image conversion and image-to-PDF.
- Poppler utilities for PDF merge, split and rendering.
- Ghostscript and qpdf for PDF compression and page operations.
- LibreOffice for document and spreadsheet conversion.
- Tesseract and language data for OCR.
- ExifTool for EXIF extraction.
- FFmpeg for video, audio and GIF features (optional).
- `pdftk` for PDF watermarking.
- `rembg` and its Python model environment for background removal.

AppImage also relies on the host's supported Linux kernel and glibc baseline. It is not a replacement for a fully self-contained virtual machine.

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

Choose an RSA signing key (3072 bits or stronger), give it a clear ForgeBox release identity, and leave the passphrase empty so the isolated CI runner can sign without an interactive prompt. This key is for package signing only.

Export the secret key to a local file:

```bash
FINGERPRINT="$(gpg --with-colons --list-secret-keys | awk -F: '$1 == "fpr" { print $10; exit }')"
gpg --armor --export-secret-keys "$FINGERPRINT" > forgebox-rpm-signing-private.asc
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

The Linux workflow runs tests and builds RPM, DEB, AppImage and Pacman outputs. A tag matching the version in `Cargo.toml` publishes the artifacts and a `SHA256SUMS` file as a GitHub Release.

The current workflow does not publish to a signed APT or Pacman repository. DEB and Arch users install the downloaded release artifacts directly. AUR/Flathub-style repository publication can be added separately after the first package builds are verified.
