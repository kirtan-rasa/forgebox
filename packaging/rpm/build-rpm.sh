#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BIN="$ROOT/target/release/forgebox"
if [[ ! -x "$BIN" ]]; then
  echo "Release binary missing: $BIN" >&2
  echo "Build it first with: cargo build --release --locked" >&2
  exit 2
fi

cargo_version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT/Cargo.toml" | head -n1)"
if [[ -z "$cargo_version" ]]; then
  echo "Could not read package version from Cargo.toml" >&2
  exit 2
fi
if [[ "$cargo_version" == *-* ]]; then
  rpm_version="${cargo_version%%-*}"
  rpm_release="1.${cargo_version#*-}"
else
  rpm_version="$cargo_version"
  rpm_release="1"
fi

TOP="$ROOT/target/rpmbuild"
rm -rf "$TOP"
mkdir -p "$TOP"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS} "$ROOT/dist"
# Prevent an older RPM from making a failed rebuild look successful.
rm -f "$ROOT"/dist/*.rpm
install -m 0755 "$BIN" "$TOP/SOURCES/forgebox"
install -m 0644 "$ROOT/assets/forgebox.png" "$TOP/SOURCES/forgebox.png"
install -m 0644 "$ROOT/share/forgebox.desktop" "$TOP/SOURCES/forgebox.desktop"
install -m 0644 "$ROOT/LICENSE" "$TOP/SOURCES/LICENSE"
install -m 0644 "$ROOT/README.md" "$TOP/SOURCES/README.md"

rpmbuild -bb "$ROOT/packaging/rpm/forgebox.spec" \
  --define "_topdir $TOP" \
  --define "forgebox_version $rpm_version" \
  --define "forgebox_release $rpm_release"

find "$TOP/RPMS" -type f -name '*.rpm' -exec cp -f {} "$ROOT/dist/" \;
if ! compgen -G "$ROOT/dist/*.rpm" >/dev/null; then
  echo "rpmbuild completed but no RPM was found." >&2
  exit 3
fi
printf 'RPM created in %s/dist\n' "$ROOT"
