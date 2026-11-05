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

name_pattern = re.compile(r"(?m)^pkgname=[^\n]+$")
if len(name_pattern.findall(text)) != 1:
    raise SystemExit("Expected exactly one pkgname assignment in generated PKGBUILD.")
text = name_pattern.sub("pkgname=forgebox", text, count=1)

package_pattern = re.compile(r"(?ms)^package\(\)\s*\{\n.*?^\}")
matches = list(package_pattern.finditer(text))
if len(matches) != 1:
    raise SystemExit("Expected exactly one package() function in generated PKGBUILD.")

replacement = """package() {
  install -d "${pkgdir}"
  cp -a "${srcdir}/usr" "${pkgdir}/"
}"""
text = package_pattern.sub(lambda _: replacement, text, count=1)

path.write_text(text, encoding="utf-8")
PY

bash -n "$PKGBUILD"
grep -q '^pkgname=forgebox$' "$PKGBUILD"
grep -Fq 'cp -a "${srcdir}/usr" "${pkgdir}/"' "$PKGBUILD"
echo "Prepared Arch PKGBUILD: package name forgebox; only the installed usr/ tree is packaged."
