# Signed pacman repository

Arch Forge maintains an independent **Arch Linux x86_64** binary repository alongside GitHub Releases. Both `archforge` (desktop + CLI) and `archforge-cli` (terminal-only alternative) are available. Installing both is unnecessary and the packages conflict over CLI ownership.

Repository and setup page: https://zynokss.github.io/archforge/

Primary signing fingerprint, pinned in `packaging/repository/fingerprint.txt`:

```text
F67F87FE838F8D8FCE0B29CFA6B8E7BF2C9A9D85
```

## One-time setup

Download the **public** key and require its primary fingerprint to match before trusting it:

```sh
curl -fsSLo archforge-repo.asc https://zynokss.github.io/archforge/archforge-repo.asc
test "$(gpg --show-keys --with-colons archforge-repo.asc | awk -F: '$1 == "fpr" { print $10; exit }')" = "F67F87FE838F8D8FCE0B29CFA6B8E7BF2C9A9D85" || exit 1
sudo pacman-key --add archforge-repo.asc
sudo pacman-key --lsign-key F67F87FE838F8D8FCE0B29CFA6B8E7BF2C9A9D85
```

Append this below the official repositories in `/etc/pacman.conf`:

```ini
[archforge]
SigLevel = Required TrustedOnly
Server = https://zynokss.github.io/archforge/repo/$arch
```

Install the full edition:

```sh
sudo pacman -Syu archforge
```

Or install the standalone alternative with `sudo pacman -Syu archforge-cli`. After setup, normal `sudo pacman -Syu` updates the installed edition. Both package and database signatures are required; do not replace this policy with `Optional TrustAll` or disable signature checking.

No setup step is performed automatically by the app or by the publisher. Removing this repository configuration stops its future updates without uninstalling the package.

## Publishing and trust boundary

- `.github/workflows/pacman-repository.yml` runs after successful future releases through the reusable workflow, or manually via `gh workflow run pacman-repository.yml --ref main`.
- `scripts/fetch-repository-packages.py` downloads versioned installers from the latest stable GitHub release and up to two previous stable releases, checking each release's SHA256SUMS. Stable filename aliases are not repository package identities.
- `scripts/build-pacman-repository.py` checks package identity/architecture/version pairing, signs the packages, runs `repo-add`, signs database/file-list archives and verifies all signatures against the pinned primary fingerprint. It indexes only the newest pair and retains older package files for clients holding older metadata.
- Pages deploys one complete snapshot. Database aliases are regular files, not symlinks. Publication refuses a missing key, invalid package, mismatched pair or a release that changed while the snapshot was being built.
- The private primary key remains in the original maintainer's private data directory, `~/.local/share/archforge-repository-signing/` by default. **Back up this directory securely**, including its revocation certificate and passphrase; never commit or upload it. Other clones need no private signing files.
- GitHub Actions secrets `ARCHFORGE_REPO_SIGNING_KEY` and `ARCHFORGE_REPO_SIGNING_PASSPHRASE` contain only the encrypted signing-subkey export and its passphrase. The CI export cannot certify keys; the signing subkey expires two years after creation. CI keyrings live outside the published artifact and are removed after signing.
- Before subkey expiry, extend/replace it using the locally retained primary key, update the committed public key and CI subkey export, then rebuild the repository. Users must re-import the updated public key when subkey certificates change; the primary fingerprint remains the same. A primary-key rotation needs a newly pinned fingerprint and user trust instructions; do not silently swap keys.

GitHub release downloads and the existing in-app updater remain available. The in-app updater checks GitHub digest/package metadata; the signed repository path uses pacman/OpenPGP verification. No pacman system files are relocated or app functionality changed by this publishing setup.

## Verification

`python scripts/test_pacman_repository.py` uses throwaway keys and fixture packages, checks signature/tamper/version failures, and exercises pacman sync/download in isolated temporary paths under fakeroot. It never installs a package or changes the host keyring/database. Requires Python, GnuPG, pacman/repo-add, bsdtar and fakeroot.

After deployment, check the live database and package signatures with the public key in a temporary keyring, and confirm pacman sync/download succeeds under `Required TrustedOnly`. The production signing key must never appear in a source archive, release asset, Pages artifact, log or repository commit.
