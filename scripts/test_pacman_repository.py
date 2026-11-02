#!/usr/bin/env python3
"""Cryptographic repository integration checks using throwaway keys and packages."""
import importlib.util
import io
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("repository_builder", Path(__file__).with_name("build-pacman-repository.py"))
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)


class SignedRepository(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory(prefix="archforge-repository-test-")
        cls.root = Path(cls.temporary.name)
        cls.home = cls.root / "gnupg"
        cls.home.mkdir(mode=0o700)
        cls.environment = patch.dict(os.environ, GNUPGHOME=str(cls.home))
        cls.environment.start()
        cls.passphrase = cls.root / "passphrase"
        cls.passphrase.write_text("throwaway-test-passphrase")
        cls.passphrase.chmod(0o600)
        subprocess.run(["gpg", "--batch", "--pinentry-mode", "loopback", "--passphrase-file", str(cls.passphrase), "--quick-generate-key", "Repository integration fixture", "ed25519", "sign", "1d"], check=True, capture_output=True)
        cls.public = cls.root / "public.asc"
        cls.public.write_text(builder.gpg("--armor", "--export"))
        cls.fingerprint = builder.fingerprint(cls.public)

    @classmethod
    def tearDownClass(cls):
        subprocess.run(["gpgconf", "--homedir", str(cls.home), "--kill", "all"], capture_output=True)
        cls.environment.stop()
        cls.temporary.cleanup()

    def packages(self, folder, mismatch=False):
        folder.mkdir()
        for name in ["archforge", "archforge-cli"]:
            versions = ["1.0.0-1", "1.0.1-1"]
            if mismatch and name == "archforge-cli":
                versions = ["1.0.0-1"]
            for version in versions:
                path = folder / f"{name}-{version}-x86_64.pkg.tar.zst"
                with tarfile.open(path, "w") as archive:
                    text = f"pkgname = {name}\npkgver = {version}\npkgdesc = Throwaway repository fixture\narch = x86_64\nsize = 1\nbuilddate = 1\n"
                    info = tarfile.TarInfo(".PKGINFO")
                    data = text.encode()
                    info.size = len(data)
                    archive.addfile(info, io.BytesIO(data))
                    info = tarfile.TarInfo("usr/share/archforge-fixture")
                    info.size = 1
                    archive.addfile(info, io.BytesIO(b"x"))
        return folder

    def test_signatures_aliases_newest_versions_and_tamper_rejection(self):
        folder = self.root / "valid"
        folder.mkdir()
        packages = self.packages(folder / "input")
        output = folder / "output"
        builder.build(packages, output, self.public, self.fingerprint, self.passphrase)
        repo = output / "repo/x86_64"
        for file in repo.iterdir():
            self.assertFalse(file.is_symlink())
            if file.name.endswith(".sig"):
                builder.verify(Path(str(file)[:-4]), self.fingerprint)
        with tarfile.open(repo / "archforge.db") as database:
            descriptions = [database.extractfile(m).read().decode() for m in database if m.name.endswith("/desc")]
        self.assertEqual(len(descriptions), 2)
        self.assertTrue(all("%VERSION%\n1.0.1-1\n" in d for d in descriptions))
        self.assertTrue(all("%PGPSIG%" in d for d in descriptions))
        self.assertIn("Required TrustedOnly", (output / "archforge.conf").read_text())
        self.assertFalse(any(b"PRIVATE KEY" in p.read_bytes() for p in output.rglob("*") if p.is_file()))
        # fakeroot grants no real privileges: every pacman state path is temporary,
        # and -w downloads only. No host databases, keys or installed files change.
        database=folder/"pacman-db";database.mkdir()
        cache=folder/"pacman-cache";cache.mkdir()
        conf=folder/"pacman.conf"
        conf.write_text(f"[options]\nArchitecture = x86_64\nDBPath = {database}\nCacheDir = {cache}\nGPGDir = {self.home}\nLogFile = {folder/'pacman.log'}\nSigLevel = Required TrustedOnly\n[archforge]\nServer = file://{repo}\n")
        command=["fakeroot","pacman","--config",str(conf),"--noconfirm"]
        for operation in [["-Sy"],["-Sddw","archforge-cli"]]:
            result=subprocess.run(command+operation,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
            self.assertEqual(result.returncode,0,result.stdout)
        self.assertTrue((cache/"archforge-cli-1.0.1-1-x86_64.pkg.tar.zst").is_file())
        (repo / "archforge.db").write_bytes(b"tampered")
        with self.assertRaises(subprocess.CalledProcessError):
            builder.verify(repo / "archforge.db", self.fingerprint)
        result=subprocess.run(command+["-Syy"],text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
        self.assertNotEqual(result.returncode,0,"pacman must reject the tampered signed database")
        package = next(repo.glob("*.pkg.tar.zst"))
        package.write_bytes(b"tampered")
        with self.assertRaises(subprocess.CalledProcessError):
            builder.verify(package, self.fingerprint)

    def test_wrong_key_and_mismatched_editions_fail_before_output(self):
        folder = self.root / "invalid"
        folder.mkdir()
        packages = self.packages(folder / "input", mismatch=True)
        output = folder / "output"
        with self.assertRaises(ValueError):
            builder.build(packages, output, self.public, "A" * 40, self.passphrase)
        with self.assertRaisesRegex(ValueError, "versions must agree"):
            builder.build(packages, output, self.public, self.fingerprint, self.passphrase)
        self.assertFalse(output.exists())

    def test_rejects_unexpected_package_metadata(self):
        folder = self.root / "unexpected"
        folder.mkdir()
        path = folder / "other-1.0.0-1-x86_64.pkg.tar.zst"
        path.write_bytes(b"not a package")
        with self.assertRaises(ValueError):
            builder.metadata(path)


if __name__ == "__main__":
    unittest.main()
