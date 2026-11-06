#!/usr/bin/env bash
set -euo pipefail

DIST_DIR="${1:-dist}"
: "${RPM_SIGNING_PRIVATE_KEY:?RPM_SIGNING_PRIVATE_KEY must contain the dedicated, ASCII-armored ForgeBox signing key}"

if ! command -v rpmsign >/dev/null 2>&1 || ! command -v gpg >/dev/null 2>&1; then
  echo "rpmsign and gpg are required to sign RPM packages." >&2
  exit 2
fi
if [[ ! -d "$DIST_DIR" ]]; then
  echo "RPM output directory not found: $DIST_DIR" >&2
  exit 2
fi

mapfile -d '' RPM_FILES < <(find "$DIST_DIR" -maxdepth 1 -type f -name '*.rpm' -print0)
if (( ${#RPM_FILES[@]} == 0 )); then
  echo "No RPM files found in $DIST_DIR." >&2
  exit 2
fi

umask 077
GNUPGHOME="$(mktemp -d)"
export GNUPGHOME
trap 'rm -rf "$GNUPGHOME"' EXIT
chmod 700 "$GNUPGHOME"

# RPM reads per-user macros from ~/.rpmmacros. Isolate HOME as well as
# GNUPGHOME so this script never overwrites the developer's RPM configuration
# or leaves a macro file pointing at the temporary keyring after cleanup.
export HOME="$GNUPGHOME"

printf '%s\n' "$RPM_SIGNING_PRIVATE_KEY" | gpg --batch --import
FINGERPRINT="$(gpg --batch --with-colons --list-secret-keys | awk -F: '$1 == "fpr" { print $10; exit }')"
if [[ -z "$FINGERPRINT" ]]; then
  echo "Could not identify the imported RPM signing key." >&2
  exit 3
fi

# Keep compatibility with both current RPM signing macros and older Fedora builds.
cat > "$HOME/.rpmmacros" <<EOF
%_gpg_path $GNUPGHOME
%_gpg_name $FINGERPRINT
%_openpgp_sign_id $FINGERPRINT
EOF

PUBLIC_KEY="$DIST_DIR/forgebox-rpm-signing-key.asc"
gpg --batch --armor --export "$FINGERPRINT" > "$PUBLIC_KEY"

# Import the verification key into a temporary RPM database. Signing a
# package should not permanently add a new trusted key to the host system.
RPMDB="$GNUPGHOME/rpmdb"
mkdir -p "$RPMDB"
rpm --dbpath "$RPMDB" --initdb
rpm --dbpath "$RPMDB" --import "$PUBLIC_KEY"

rpmsign --addsign "${RPM_FILES[@]}"
for package in "${RPM_FILES[@]}"; do
  rpmkeys --dbpath "$RPMDB" --checksig --verbose "$package"
done

printf '%s\n' "$FINGERPRINT" > "$DIST_DIR/forgebox-rpm-signing-key-id.txt"
echo "Signed and verified ${#RPM_FILES[@]} RPM package(s)."
