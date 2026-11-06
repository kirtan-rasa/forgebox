#!/usr/bin/env sh
set -eu

# Installs the newest ForgeBox release using the distribution's package manager.
# Compatible with: curl -f https://kirtan-rasa.github.io/forgebox/install.sh | sh

RELEASES_API="https://api.github.com/repos/kirtan-rasa/forgebox/releases?per_page=100"
INSTALL_URL="https://kirtan-rasa.github.io/forgebox/install.sh"

fail() {
    printf 'ForgeBox installer: %s\n' "$1" >&2
    exit 1
}

run_privileged() {
    if [ "$(id -u)" = "0" ]; then
        "$@"
    else
        command -v sudo >/dev/null 2>&1 || fail "sudo is required. Run the installer as root or install sudo."
        sudo "$@"
    fi
}

command -v curl >/dev/null 2>&1 || fail "curl is required."
command -v sha256sum >/dev/null 2>&1 || fail "sha256sum is required."

ARCH="$(uname -m)"
[ "$ARCH" = "x86_64" ] || fail "This release currently supports x86_64 systems only. Detected: $ARCH."

[ -r /etc/os-release ] || fail "Cannot identify this Linux distribution (/etc/os-release is missing)."
# shellcheck disable=SC1091
. /etc/os-release
ID_LIKE=${ID_LIKE-}
VERSION_ID=${VERSION_ID-unknown}
PRETTY_NAME=${PRETTY_NAME-${ID-unknown}}

PACKAGE_FAMILY=""
case "${ID-}" in
    fedora)
        PACKAGE_FAMILY="fedora"
        ;;
    debian|ubuntu|linuxmint|pop|elementary)
        PACKAGE_FAMILY="debian"
        ;;
    arch|manjaro|endeavouros)
        PACKAGE_FAMILY="arch"
        ;;
    *)
        case " $ID_LIKE " in
            *" fedora "*)
                PACKAGE_FAMILY="fedora"
                ;;
            *" debian "*)
                PACKAGE_FAMILY="debian"
                ;;
            *" arch "*)
                PACKAGE_FAMILY="arch"
                ;;
            *)
                fail "Unsupported distribution: $PRETTY_NAME. Supported families: Fedora, Debian/Ubuntu, and Arch Linux."
                ;;
        esac
        ;;
esac

case "$PACKAGE_FAMILY" in
    fedora)
        command -v dnf >/dev/null 2>&1 || fail "DNF was not found on this Fedora-based system."
        ;;
    debian)
        command -v apt-get >/dev/null 2>&1 || fail "APT was not found on this Debian-based system."
        ;;
    arch)
        command -v pacman >/dev/null 2>&1 || fail "Pacman was not found on this Arch-based system."
        ;;
esac

if ! command -v python3 >/dev/null 2>&1; then
    printf 'Python 3 is needed to read GitHub release metadata. Installing it first...\n'
    case "$PACKAGE_FAMILY" in
        fedora)
            run_privileged dnf install --assumeyes python3
            ;;
        debian)
            run_privileged apt-get update
            run_privileged apt-get install --yes python3
            ;;
        arch)
            run_privileged pacman -S --needed --noconfirm python
            ;;
    esac
fi
command -v python3 >/dev/null 2>&1 || fail "Python 3 could not be installed."

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/forgebox-install.XXXXXX")"
trap 'rm -rf "$TMP_DIR"' 0
trap 'exit 1' HUP INT TERM

printf 'Checking the latest ForgeBox release for %s...\n' "$PRETTY_NAME"
curl --fail --location --silent --show-error --retry 3 \
    --header 'Accept: application/vnd.github+json' \
    --header 'X-GitHub-Api-Version: 2022-11-28' \
    --user-agent 'ForgeBox-Installer' \
    --output "$TMP_DIR/releases.json" \
    "$RELEASES_API"

python3 - "$TMP_DIR/releases.json" "$PACKAGE_FAMILY" "$VERSION_ID" > "$TMP_DIR/asset-info" <<'PY'
import json
import os
import re
import sys

json_path, family, version = sys.argv[1:]
try:
    with open(json_path, "r", encoding="utf-8") as source:
        releases = json.load(source)
except (OSError, json.JSONDecodeError) as error:
    print(f"Could not parse GitHub release metadata: {error}", file=sys.stderr)
    sys.exit(1)

if not isinstance(releases, list):
    print("GitHub returned an unexpected release list.", file=sys.stderr)
    sys.exit(1)

releases = [release for release in releases if isinstance(release, dict)]
releases.sort(key=lambda release: release.get("published_at") or "", reverse=True)

for release in releases:
    if release.get("draft"):
        continue

    for asset in release.get("assets", []):
        if not isinstance(asset, dict):
            continue
        name = asset.get("name") or ""

        if family == "fedora":
            matches = name.endswith(f".fc{version}.x86_64.rpm")
        elif family == "debian":
            matches = name.endswith("_amd64.deb")
        elif family == "arch":
            matches = name.endswith("-x86_64.pkg.tar.zst")
        else:
            matches = False

        if not matches:
            continue

        url = asset.get("browser_download_url") or ""
        expected_prefix = "https://github.com/kirtan-rasa/forgebox/releases/download/"
        if os.path.basename(name) != name or not url.startswith(expected_prefix):
            print("GitHub returned an unexpected package asset URL.", file=sys.stderr)
            sys.exit(1)

        digest = asset.get("digest") or ""
        if not re.fullmatch(r"sha256:[0-9a-fA-F]{64}", digest):
            print(
                f"The package {name} has no valid SHA-256 digest. "
                "Refusing to install an unverified file.",
                file=sys.stderr,
            )
            sys.exit(1)

        print(release.get("tag_name", "unknown"))
        print(name)
        print(url)
        print(digest.split(":", 1)[1])
        sys.exit(0)

print(
    f"No published ForgeBox package was found for {family} "
    f"(version {version}, x86_64).",
    file=sys.stderr,
)
sys.exit(1)
PY

RELEASE_TAG="$(sed -n '1p' "$TMP_DIR/asset-info")"
PACKAGE_NAME="$(sed -n '2p' "$TMP_DIR/asset-info")"
PACKAGE_URL="$(sed -n '3p' "$TMP_DIR/asset-info")"
EXPECTED_SHA256="$(sed -n '4p' "$TMP_DIR/asset-info")"

[ -n "$RELEASE_TAG" ] || fail "GitHub did not provide a release tag."
[ -n "$PACKAGE_NAME" ] || fail "GitHub did not provide a package filename."
[ -n "$PACKAGE_URL" ] || fail "GitHub did not provide a package URL."
[ -n "$EXPECTED_SHA256" ] || fail "GitHub did not provide a package digest."

PACKAGE_PATH="$TMP_DIR/$PACKAGE_NAME"
printf 'Selected release: %s\nPackage: %s\nDownloading and verifying SHA-256...\n' "$RELEASE_TAG" "$PACKAGE_NAME"
curl --fail --location --silent --show-error --retry 3 \
    --output "$PACKAGE_PATH" "$PACKAGE_URL"

[ -s "$PACKAGE_PATH" ] || fail "The downloaded package is empty."
ACTUAL_SHA256="$(sha256sum "$PACKAGE_PATH" | awk '{print $1}')"
[ "$ACTUAL_SHA256" = "$EXPECTED_SHA256" ] || fail "SHA-256 verification failed. The package will not be installed."
printf 'SHA-256 verified.\n'

printf '\nInstalling ForgeBox with the native package manager...\n'
printf 'The package manager will install ForgeBox and its declared dependencies. sudo may ask for your password.\n'

case "$PACKAGE_FAMILY" in
    fedora)
        run_privileged dnf install --assumeyes "$PACKAGE_PATH"
        ;;
    debian)
        run_privileged apt-get update
        run_privileged apt-get install --yes "$PACKAGE_PATH"
        ;;
    arch)
        if ! run_privileged pacman -U --noconfirm "$PACKAGE_PATH"; then
            printf '\nPacman could not resolve the package dependencies. Arch systems should be fully updated before installing packages.\n' >&2
            printf 'Run: sudo pacman -Syu\nThen rerun the ForgeBox installer.\n' >&2
            exit 1
        fi
        ;;
esac

printf '\nForgeBox %s was installed successfully.\n' "$RELEASE_TAG"
printf 'Launch it from your application menu or run: forgebox\n'
