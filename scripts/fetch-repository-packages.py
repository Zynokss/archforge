#!/usr/bin/env python3
"""Fetch checksum-verified, versioned release packages; no signing credentials needed."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

REPOSITORY = "zynoksy/archforge"
TAG = re.compile(r"v\d+\.\d+\.\d+\Z")


def api(path):
    return json.loads(subprocess.check_output(["gh", "api", path], text=True))


def fetch(destination):
    destination.mkdir(parents=True, exist_ok=False)
    latest = api(f"repos/{REPOSITORY}/releases/latest")
    if not TAG.fullmatch(latest["tag_name"]):
        raise ValueError("Latest stable release must have a version tag")
    releases = api(f"repos/{REPOSITORY}/releases?per_page=30")
    releases = [latest] + [r for r in releases if r["id"] != latest["id"] and not r["draft"] and not r["prerelease"] and TAG.fullmatch(r["tag_name"])][:2]
    for release in releases:
        version = release["tag_name"][1:]
        names = [a["name"] for a in release["assets"] if re.fullmatch(rf"archforge(?:-cli)?-{re.escape(version)}-\d+-x86_64\.pkg\.tar\.zst", a["name"])]
        if len(names) != 2 or sum(n.startswith("archforge-cli-") for n in names) != 1:
            if release["id"] == latest["id"]:
                raise ValueError("Latest release must contain both versioned installers")
            continue
        folder = destination / release["tag_name"]
        folder.mkdir()
        for name in ["SHA256SUMS", *names]:
            subprocess.run(["gh", "release", "download", release["tag_name"], "--repo", REPOSITORY, "--pattern", name, "--dir", str(folder)], check=True)
        checksums = {}
        for line in (folder / "SHA256SUMS").read_text().splitlines():
            digest, name = line.split(maxsplit=1)
            if not re.fullmatch(r"[0-9a-f]{64}", digest):
                raise ValueError("Malformed release checksum")
            checksums[name.lstrip(" *")] = digest
        for name in names:
            package = folder / name
            digest = hashlib.sha256(package.read_bytes()).hexdigest()
            if checksums.get(name) != digest:
                raise ValueError(f"Release checksum mismatch: {name}")
            package.rename(destination / name)
        (folder / "SHA256SUMS").unlink()
        folder.rmdir()
    (destination / "release-tag.txt").write_text(latest["tag_name"] + "\n")
    print(f"Verified packages for {latest['tag_name']} and up to two previous releases")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", type=Path)
    fetch(parser.parse_args().destination)
