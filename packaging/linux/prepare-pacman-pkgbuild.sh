#!/usr/bin/env bash
set -euo pipefail

PKGBUILD="${1:-dist/linux/PKGBUILD}"
if [[ ! -f "$PKGBUILD" ]]; then
  echo "PKGBUILD not found: $PKGBUILD" >&2
  exit 2
fi

python3 - "$PKGBUILD" <<'PY'
from pathlib import Path
import re
import sys

path = Path(sys.argv[1])
text = path.read_text(encoding="utf-8")

def replace_one(pattern: str, replacement: str, label: str) -> None:
    global text
    regex = re.compile(pattern, re.MULTILINE)
    matches = list(regex.finditer(text))
    if len(matches) != 1:
        raise SystemExit(f"Expected exactly one {label} assignment/function in generated PKGBUILD.")
    text = regex.sub(replacement, text, count=1)

replace_one(r"^pkgname=[^\n]+$", "pkgname=forgebox", "pkgname")

version_match = re.search(r"(?m)^pkgver=([^\n]+)$", text)
if not version_match:
    raise SystemExit("Generated PKGBUILD does not contain pkgver.")
pkgver = version_match.group(1).strip().strip("\\\"'")
if not pkgver:
    raise SystemExit("Generated PKGBUILD has an empty pkgver.")
# Arch makepkg forbids hyphens in pkgver. Keep prerelease identity with underscores.
arch_version = pkgver.replace("-", "_")
if any(char.isspace() for char in arch_version) or "/" in arch_version or ":" in arch_version:
    raise SystemExit(f"Generated version cannot be represented as an Arch pkgver: {pkgver}")
replace_one(r"^pkgver=[^\n]+$", f"pkgver={arch_version}", "pkgver")

package_pattern = r"(?ms)^package\(\)\s*\{\n.*?^\}"
replacement = """package() {
  install -d "${pkgdir}"
  cp -a "${srcdir}/usr" "${pkgdir}/"
}"""
replace_one(package_pattern, replacement, "package()")

# FFmpeg is intentionally optional: the image, PDF, document, and data tools do
# not need the sizeable audio/video backend. cargo-packager does not currently
# expose Pacman optdepends, so add the supported media backend to PKGBUILD here.
optional_dependency = "ffmpeg: Enable audio, video, and GIF conversion"
if re.search(r"(?m)^optdepends=", text):
    optdepends_match = re.search(r"(?ms)^optdepends=\((.*?)\)", text)
    if not optdepends_match:
        raise SystemExit("Could not parse generated optdepends array.")
    if optional_dependency not in optdepends_match.group(1):
        insertion = optdepends_match.end() - 1
        text = text[:insertion] + f"\n  '{optional_dependency}'" + text[insertion:]
else:
    depends_start = re.search(r"(?m)^depends=\(", text)
    if not depends_start:
        raise SystemExit("Generated PKGBUILD does not contain depends.")
    depth = 0
    depends_end = None
    for index in range(depends_start.start(), len(text)):
        if text[index] == "(":
            depth += 1
        elif text[index] == ")":
            depth -= 1
            if depth == 0:
                depends_end = index + 1
                break
    if depends_end is None:
        raise SystemExit("Could not find the end of the generated depends array.")
    text = text[:depends_end] + f"\noptdepends=('{optional_dependency}')" + text[depends_end:]

path.write_text(text, encoding="utf-8")
PY

bash -n "$PKGBUILD"
grep -q '^pkgname=forgebox$' "$PKGBUILD"
grep -Eq '^pkgver=[^[:space:]/:-]+$' "$PKGBUILD"
grep -Fq 'cp -a "${srcdir}/usr" "${pkgdir}/"' "$PKGBUILD"
echo "Prepared Arch PKGBUILD: normalized version, package name forgebox, and clean usr/ payload."
