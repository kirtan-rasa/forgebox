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

path.write_text(text, encoding="utf-8")
PY

bash -n "$PKGBUILD"
grep -q '^pkgname=forgebox$' "$PKGBUILD"
grep -Eq '^pkgver=[^[:space:]/:-]+$' "$PKGBUILD"
grep -Fq 'cp -a "${srcdir}/usr" "${pkgdir}/"' "$PKGBUILD"
echo "Prepared Arch PKGBUILD: normalized version, package name forgebox, and clean usr/ payload."
