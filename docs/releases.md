# Releases

OnTrack publishes versioned binaries from `.github/workflows/release.yml`. A pushed tag in the form `vMAJOR.MINOR.PATCH` must exactly match the `version` in `Cargo.toml`. For example, version `0.1.0` uses tag `v0.1.0`. Update the package version and lockfile, commit the change, then create and push the tag on that commit. A pushed tag starts the workflow; no GitHub Release is published until its checks and all builds pass.

## Supported downloads

| System | CPU | Rust target |
| --- | --- | --- |
| Linux (Ubuntu 24.04 compatible) | x86-64 | `x86_64-unknown-linux-gnu` |
| Linux (Ubuntu 24.04 compatible) | ARM64 | `aarch64-unknown-linux-gnu` |
| macOS | Intel | `x86_64-apple-darwin` |
| macOS | Apple Silicon | `aarch64-apple-darwin` |

For each target, a release contains `ontrack-vVERSION-TARGET.tar.gz` and a matching `ontrack-vVERSION-TARGET.tar.gz.sha256`. The archive contains one executable named `ontrack`. The checksum file uses the standard `SHA256  filename` format and hashes the downloadable archive, not the unpacked binary. These names are the contract for future installers and update features.

Download the archive and its `.sha256` file from the same [GitHub Release](https://github.com/ThomasJRyan/OnTrack/releases). In the download directory, verify before unpacking:

```sh
# Linux
sha256sum -c ontrack-vVERSION-TARGET.tar.gz.sha256

# macOS
shasum -a 256 -c ontrack-vVERSION-TARGET.tar.gz.sha256

tar -xzf ontrack-vVERSION-TARGET.tar.gz
mkdir -p "$HOME/.local/bin"
install -m 755 ontrack "$HOME/.local/bin/ontrack"
```

Replace `VERSION` and `TARGET` with the downloaded release values, and ensure `$HOME/.local/bin` exists and is on your `PATH`. OnTrack also needs the [`bd` command](https://beads.gascity.com) to open a workspace. Its agent handoff features need a working [Herdr installation](https://herdr.dev/docs).

## Maintainer process

The release workflow first verifies the tag and runs formatting, tests, Clippy, a normal build, and release packaging tests. Four native GitHub-hosted runners then build the listed targets and upload archives with their checksums. The publish job downloads all four sets, requires the exact expected filenames, recalculates every SHA-256 digest, and creates a GitHub Release with all eight assets. A failed check, build, upload, or verification prevents the publish job from running. GitHub CLI creates a draft while uploading assets and publishes it only after uploads succeed.

For a local packaging check, run:

```sh
python3 -m unittest discover -s scripts -p 'test_release_asset.py'
python3 scripts/release_asset.py validate-tag v0.1.0
```

Use the current package version for the second command. GitHub Actions is the authority for actual native platform builds and publishing; local checks cannot validate those remote steps.
