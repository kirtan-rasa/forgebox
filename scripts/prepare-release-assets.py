#!/usr/bin/env python3
"""Select the small, explicit set of files attached to a public ForgeBox release."""

from __future__ import annotations

import argparse
import shutil
import sys
import tempfile
from pathlib import Path
from typing import Callable


AssetRule = tuple[str, Callable[[Path], bool]]

ASSET_RULES: list[AssetRule] = [
    ("Fedora RPM", lambda path: path.suffix == ".rpm"),
    ("Debian package", lambda path: path.suffix == ".deb"),
    ("AppImage", lambda path: path.suffix.lower() == ".appimage"),
    (
        "Arch Pacman package",
        lambda path: path.name.startswith("forgebox-")
        and path.name.endswith(".pkg.tar.zst")
        and not path.name.startswith("forgebox-debug-"),
    ),
    (
        "RPM public signing key",
        lambda path: path.name == "forgebox-rpm-signing-key.asc",
    ),
]


def select_assets(source: Path) -> list[Path]:
    """Return exactly one file for every required public release asset."""
    if not source.is_dir():
        raise FileNotFoundError(f"Release artifact directory not found: {source}")

    files = [path for path in source.iterdir() if path.is_file()]
    selected: list[Path] = []

    for label, matches_asset in ASSET_RULES:
        matches = sorted(path for path in files if matches_asset(path))
        if len(matches) != 1:
            raise ValueError(
                f"Expected exactly one {label} in CI artifacts, found {len(matches)}."
            )
        selected.append(matches[0])

    return selected


def prepare_release_assets(source: Path, destination: Path) -> list[Path]:
    """Copy only the validated release assets into a clean output directory."""
    source = source.resolve()
    destination = destination.resolve()
    if source == destination or source in destination.parents:
        raise ValueError("The public release directory must be separate from the source artifacts.")

    selected = select_assets(source)

    if destination.exists():
        if not destination.is_dir():
            raise NotADirectoryError(f"Release output path is not a directory: {destination}")
        shutil.rmtree(destination)
    destination.mkdir(parents=True)

    copied: list[Path] = []
    for asset in selected:
        target = destination / asset.name
        shutil.copy2(asset, target)
        copied.append(target)

    return copied


def self_test() -> None:
    """Exercise the allowlist against realistic package and build-output names."""
    expected = {
        "forgebox-0.5.0-1.fc45.x86_64.rpm",
        "forgebox_0.5.0_amd64.deb",
        "forgebox_0.5.0_x86_64.AppImage",
        "forgebox-0.5.0-1-x86_64.pkg.tar.zst",
        "forgebox-rpm-signing-key.asc",
    }
    ignored = {
        "forgebox-debug-0.5.0-1-x86_64.pkg.tar.zst",
        "forgebox-rpm-signing-key-id.txt",
        "PKGBUILD",
        ".SRCINFO",
        "default.SRCINFO",
        "forgebox_0.5.0_x86_64.tar.gz",
    }

    with tempfile.TemporaryDirectory(prefix="forgebox-release-assets-test-") as temp:
        root = Path(temp)
        source = root / "source"
        destination = root / "public"
        source.mkdir()
        for name in expected | ignored:
            (source / name).write_text("fixture", encoding="utf-8")

        copied = prepare_release_assets(source, destination)
        actual = {path.name for path in copied}
        if actual != expected:
            raise AssertionError(
                f"Release asset allowlist mismatch. Expected {sorted(expected)}, got {sorted(actual)}."
            )
        if {path.name for path in destination.iterdir()} != expected:
            raise AssertionError("Unexpected files were copied into the public release directory.")

        # A duplicate package must fail closed instead of publishing an ambiguous asset set.
        (source / "unexpected-extra.rpm").write_text("duplicate", encoding="utf-8")
        try:
            select_assets(source)
        except ValueError:
            pass
        else:
            raise AssertionError("The asset selector accepted duplicate RPM files.")

        # A missing signing key must also fail closed.
        (source / "unexpected-extra.rpm").unlink()
        (source / "forgebox-rpm-signing-key.asc").unlink()
        try:
            select_assets(source)
        except ValueError:
            pass
        else:
            raise AssertionError("The asset selector accepted a release without its public key.")

    print("Release asset allowlist self-test passed.")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=Path("release-assets"))
    parser.add_argument("--destination", type=Path, default=Path("public-release"))
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    try:
        if args.self_test:
            self_test()
            return 0

        copied = prepare_release_assets(args.source, args.destination)
    except (OSError, ValueError, AssertionError) as error:
        print(f"Release asset preparation failed: {error}", file=sys.stderr)
        return 1

    print("Selected public release assets:")
    for path in sorted(copied):
        print(f"  {path.name}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
