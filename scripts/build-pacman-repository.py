#!/usr/bin/env python3
"""Build a fully signed static pacman repository. Never modifies system pacman state."""
import argparse
from functools import cmp_to_key
import hashlib
import html
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

PROJECT = Path(__file__).resolve().parent.parent
BASE_URL = "https://zynokss.github.io/archforge"
NAMES = {"archforge", "archforge-cli"}


def gpg(*args):
    return subprocess.check_output(["gpg", "--batch", *map(str, args)], text=True, stderr=subprocess.PIPE)


def fingerprint(public_key):
    for line in gpg("--with-colons", "--show-keys", public_key).splitlines():
        fields = line.split(":")
        if fields[0] == "fpr":
            return fields[9]
    raise ValueError("Public key has no primary fingerprint")


def metadata(package):
    if package.is_symlink() or not re.fullmatch(r"archforge(?:-cli)?-[0-9][A-Za-z0-9.+_:~-]*-x86_64\.pkg\.tar\.zst", package.name):
        raise ValueError(f"Unexpected package path: {package.name}")
    text = subprocess.check_output(["bsdtar", "-xOf", str(package), ".PKGINFO"], text=True)
    info = dict(line.split(" = ", 1) for line in text.splitlines() if " = " in line)
    if info.get("pkgname") not in NAMES or info.get("arch") != "x86_64":
        raise ValueError("Only the two Arch Forge x86_64 packages may be signed")
    version = info.get("pkgver", "")
    if not re.fullmatch(r"\d+\.\d+\.\d+-\d+", version):
        raise ValueError("Unexpected package version")
    if package.name != f"{info['pkgname']}-{version}-x86_64.pkg.tar.zst":
        raise ValueError("Package filename and metadata disagree")
    return info


def verify(path, expected):
    status = gpg("--status-fd", "1", "--verify", str(path) + ".sig", path)
    valid = [line.split() for line in status.splitlines() if line.startswith("[GNUPG:] VALIDSIG ")]
    if not any(expected in (line[2], line[-1]) for line in valid):
        raise ValueError(f"Signature does not match the pinned key: {path.name}")


def sign(path, expected, passphrase_file):
    gpg("--yes", "--pinentry-mode", "loopback", "--passphrase-file", passphrase_file,
        "--local-user", expected, "--detach-sign", path)
    verify(path, expected)


def bootstrap(expected):
    return f'''curl -fsSLo archforge-repo.asc {BASE_URL}/archforge-repo.asc
# Verify the fingerprint before adding the key:
test "$(gpg --show-keys --with-colons archforge-repo.asc | awk -F: '$1 == "fpr" {{ print $10; exit }}')" = "{expected}" || exit 1
sudo pacman-key --add archforge-repo.asc
sudo pacman-key --lsign-key {expected}
'''


def build(packages, output, public_key, expected, passphrase_file):
    expected = expected.strip().upper()
    if not re.fullmatch(r"[0-9A-F]{40}", expected):
        raise ValueError("A full pinned OpenPGP fingerprint is required")
    if not os.environ.get("GNUPGHOME"):
        raise ValueError("Set a dedicated GNUPGHOME; never use the personal default keyring")
    if fingerprint(public_key) != expected:
        raise ValueError("Public key does not match the pinned fingerprint")
    for private_path in [Path(os.environ["GNUPGHOME"]), passphrase_file]:
        if private_path.resolve().is_relative_to(output.resolve()):
            raise ValueError("Private signing material must be outside the published directory")
    paths = sorted(packages.glob("*.pkg.tar.zst"))
    if not 2 <= len(paths) <= 6 or sum(p.stat().st_size for p in paths) > 200 * 1024 * 1024:
        raise ValueError("Expected two installers and at most two previous release pairs")
    infos = {p: metadata(p) for p in paths}
    newest = {}
    for name in sorted(NAMES):
        candidates = [p for p in paths if infos[p]["pkgname"] == name]
        if not candidates:
            raise ValueError(f"Missing installer: {name}")
        def compare(a, b):
            return int(subprocess.check_output(["vercmp", infos[a]["pkgver"], infos[b]["pkgver"]], text=True))
        newest[name] = sorted(candidates, key=cmp_to_key(compare))[-1]
    if len({infos[p]["pkgver"] for p in newest.values()}) != 1:
        raise ValueError("Full and standalone package versions must agree")
    if output.exists() and any(output.iterdir()):
        raise ValueError("Output directory must be empty to avoid publishing stale/private files")
    repo = output / "repo/x86_64"
    repo.mkdir(parents=True)
    entries = []
    for source in paths:
        target = repo / source.name
        shutil.copyfile(source, target)
        sign(target, expected, passphrase_file)
        entries.append({"filename": target.name, "sha256": hashlib.sha256(target.read_bytes()).hexdigest(), "version": infos[source]["pkgver"], "name": infos[source]["pkgname"], "current": source in newest.values()})
    subprocess.run(["repo-add", "--include-sigs", "--nocolor", str(repo / "archforge.db.tar.gz"), *[str(repo / p.name) for p in newest.values()]], check=True)
    for kind in ["db", "files"]:
        archive = repo / f"archforge.{kind}.tar.gz"
        sign(archive, expected, passphrase_file)
        alias = repo / f"archforge.{kind}"
        if alias.is_symlink():
            alias.unlink()
        shutil.copyfile(archive, alias)
        shutil.copyfile(str(archive) + ".sig", str(alias) + ".sig")
        verify(alias, expected)
    shutil.copyfile(public_key, output / "archforge-repo.asc")
    (output / "fingerprint.txt").write_text(expected + "\n")
    (output / ".nojekyll").touch()
    configuration = f"[archforge]\nSigLevel = Required TrustedOnly\nServer = {BASE_URL}/repo/$arch\n"
    (output / "archforge.conf").write_text(configuration)
    version = infos[newest["archforge"]]["pkgver"]
    (repo / "packages.json").write_text(json.dumps({"version": version, "fingerprint": expected, "packages": entries}, indent=2) + "\n")
    instructions = bootstrap(expected)
    links = "\n".join(f'<li><a href="repo/x86_64/{html.escape(e["filename"])}">{html.escape(e["filename"])}</a> (<a href="repo/x86_64/{html.escape(e["filename"])}.sig">signature</a>)</li>' for e in entries if e["current"])
    (output / "index.html").write_text(f'''<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Arch Forge signed pacman repository</title>
<style>body{{font:16px/1.6 system-ui;max-width:900px;margin:48px auto;padding:0 24px;background:#0d1015;color:#e9edf4}}a{{color:#71d9ff}}pre{{padding:18px;background:#18212b;overflow:auto;border-radius:8px}}code{{font-size:14px}}h1,h2{{line-height:1.2}}</style>
<h1>Arch Forge signed pacman repository</h1><p>Arch Linux x86_64 · current package version {html.escape(version)}. This is an independently maintained third-party repository.</p>
<p>The full app includes the desktop interface and CLI. The standalone CLI is the alternative without GTK/VTE. Choose one.</p>
<h2>1. Trust the signing key</h2><p>Primary fingerprint: <code>{expected}</code>. Compare this with the fingerprint in the <a href="https://github.com/Zynokss/archforge/blob/main/docs/PACMAN_REPOSITORY.md">source repository documentation</a>. Never import a private key.</p>
<pre><code>{html.escape(instructions)}</code></pre>
<h2>2. Add the repository</h2><p>Append this to <code>/etc/pacman.conf</code>, below the official repositories:</p><pre><code>{html.escape(configuration)}</code></pre>
<h2>3. Install or update</h2><pre><code>sudo pacman -Syu archforge</code></pre><p>For terminal-only systems, use <code>sudo pacman -Syu archforge-cli</code> instead. Subsequent updates use the normal <code>sudo pacman -Syu</code> command.</p>
<h2>Downloads</h2><ul>{links}</ul><p><a href="archforge-repo.asc">Public key</a> · <a href="repo/x86_64/archforge.db">Signed database</a> · <a href="repo/x86_64/archforge.db.sig">Database signature</a> · <a href="https://github.com/Zynokss/archforge/releases">GitHub releases</a></p></html>''')
    if any(p.is_symlink() for p in output.rglob("*")):
        raise ValueError("Pages artifacts must not contain symlinks")
    print(f"Signed and verified repository {version} with {expected}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--packages", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--public-key", type=Path, default=PROJECT / "packaging/repository/archforge-repo.asc")
    parser.add_argument("--fingerprint", default=None)
    parser.add_argument("--passphrase-file", type=Path, required=True)
    args = parser.parse_args()
    pinned = args.fingerprint or (PROJECT / "packaging/repository/fingerprint.txt").read_text().strip()
    build(args.packages, args.output, args.public_key, pinned, args.passphrase_file)
