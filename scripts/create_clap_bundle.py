import enum
import shutil
import subprocess
import sys
from argparse import ArgumentParser
from dataclasses import dataclass
from functools import cached_property
from pathlib import Path
from typing import Optional


@dataclass
class BundleClap:
    input: Path
    id: str
    name: str
    version: str

    @cached_property
    def output(self) -> Path:
        return self.input.with_stem(self.input.stem.removeprefix("lib")).with_suffix(".clap")

    @classmethod
    def from_args(cls, args: Optional[str] = None):
        parser = ArgumentParser(description="Generate .clap files")
        parser.add_argument("input", help="Input library", type=Path)
        parser.add_argument("--id", help="Bundle identifier", type=str, required=True)
        parser.add_argument("--name", help="Bundle name", type=str, required=True)
        parser.add_argument("--version", help="Bundle version", type=str, required=True)
        args = parser.parse_args(args)
        return cls(args.input, args.id, args.name, args.version)

    def run(self):
        match sys.platform:
            case "darwin":
                self.bundle_darwin()
            case _:
                self.bundle()

    def bundle(self):
        shutil.copyfile(self.input, self.output)

    def bundle_darwin(self):
        self.create_bundle()
        if not self.codesign():
            print("WARNING: Code signing failed:", self.output)
            print("It may fail to launch.")

    def create_bundle(self):
        if self.output.exists():
            shutil.rmtree(self.output)
        self.output.mkdir(parents=True)
        contents_dir = self.output / "Contents"
        contents_dir.mkdir()
        pkg_type: PackageType = PackageType.BUNDLE
        plist_path = contents_dir / "Info.plist"
        plist_path.write_text(plist(self.id, self.name, pkg_type, self.version))
        pkginfo = contents_dir / "PkgInfo"
        pkginfo.write_text(f"{pkg_type}????")

        macos_dir = contents_dir / "MacOS"
        macos_dir.mkdir()
        module = macos_dir / self.output.stem
        shutil.copyfile(self.input, module)

    def codesign(self) -> bool:
        status = subprocess.call(
            [
                "codesign",
                "-f",
                "-s",
                "-",
                str(self.output),
            ])
        return status == 0


class PackageType(enum.StrEnum):
    BUNDLE = "BNDL"
    APPLICATION = "APPL"


def plist(package_id: str, display_name: str, package_type: PackageType, version: str = "1.0.0") -> str:
    return f"""
<?xml version="1.0" encoding="UTF-8"?>

<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist>
  <dict>
    <key>CFBundleExecutable</key>
    <string>{display_name}</string>
    <key>CFBundleIconFile</key>
    <string></string>
    <key>CFBundleIdentifier</key>
    <string>{package_id}</string>
    <key>CFBundleName</key>
    <string>{display_name}</string>
    <key>CFBundleDisplayName</key>
    <string>{display_name}</string>
    <key>CFBundlePackageType</key>
    <string>{package_type}</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>CFBundleShortVersionString</key>
    <string>{version}</string>
    <key>CFBundleVersion</key>
    <string>{version}</string>
    <key>NSHumanReadableCopyright</key>
    <string></string>
    <key>NSHighResolutionCapable</key>
    <true/>
  </dict>
</plist>
"""


if __name__ == "__main__":
    BundleClap.from_args().run()
