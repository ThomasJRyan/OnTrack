# Install and update OnTrack

The installer supports macOS and Ubuntu 24.04-compatible glibc Linux on x86_64 or aarch64. It needs Bash,
`curl`, Python 3, and a writable user-owned install directory. It installs one
`ontrack` executable into `~/.local/bin` by default. Rust is needed only when
building from source. A terminal with alternate-screen support is needed to run
OnTrack.

The installer checks for [Beads (`bd`)](https://beads.gascity.com/) and
[Herdr](https://herdr.dev/docs/install/). If either command is missing, it asks
before running that project's official installer:

```sh
# Beads
curl -fsSL https://raw.githubusercontent.com/gastownhall/beads/main/scripts/install.sh | bash

# Herdr
curl -fsSL https://herdr.dev/install.sh | sh
```

Each prompt defaults to **No**. You can install either dependency yourself and
rerun the OnTrack installer. Check `bd version` and `herdr --version` after
installing. The official Beads script looks up its latest release through the
GitHub REST API, so GitHub API rate limits can prevent that dependency install;
wait for the limit to reset and rerun the installer if that happens.
OnTrack has no pinned numeric minimum for `bd` or Herdr. It needs a `bd`
version supporting the JSON commands in the
[development guide](development.md#beads-cli-contract), and Herdr integration
features described in the [user guide](user-guide.md#launch-an-agent-with-herdr).
The Herdr CLI contract in the development guide was checked with Herdr 0.9.1;
the installer checks only that the commands exist, so confirm their versions
and update either dependency through its own installation method if a required
command or response format is unavailable. For this repository's embedded Dolt
workspace, install an embedded-capable `bd` build as described in the
[Beads installation guide](https://beads.gascity.com/getting-started/installation/).

## Install

Once a stable OnTrack GitHub Release with an asset for your platform exists:

```sh
curl -fsSLo install-ontrack.sh https://raw.githubusercontent.com/ThomasJRyan/OnTrack/main/scripts/install.sh && bash install-ontrack.sh
```

The installer finds the latest release through GitHub's Releases page, then
downloads that release's versioned archive and checksum directly. It does not
call the GitHub REST API, so API rate limits do not block release discovery.
Review the downloaded script before running it if desired. It is also usable
directly in a Bash pipeline; enable `pipefail` so a script-download error fails
the whole pipeline:

```bash
set -o pipefail
curl -fsSL https://raw.githubusercontent.com/ThomasJRyan/OnTrack/main/scripts/install.sh | bash
```

To choose another location, pass an **absolute** directory path:

```sh
bash install-ontrack.sh --install-dir "$HOME/bin"
```

Add that directory to `PATH` if needed. Start `ontrack` from the Git workspace
whose Beads you want to browse; its launch directory selects the workspace.
For Herdr handoffs, start it inside a running Herdr shell as explained in the
[user guide](user-guide.md#launch-an-agent-with-herdr).

## Update and remove

From a running OnTrack session, press `u` in the issue browser to check the
latest stable release. When a newer version appears, press `i` to select an
asset for this machine, then `y` to confirm installation. This path needs
`curl`, `tar`, and write access to the directory containing the running
executable. OnTrack verifies the published SHA-256 checksum before replacing
it. `Esc` or `c` requests cancellation during download; network, permission,
and verification failures leave the existing executable usable. Quit and
restart OnTrack after success. See the [user guide](user-guide.md#update-ontrack)
for the dialog and recovery steps.

An existing destination is left untouched unless `--replace` is explicit.
Update by running the installer again with `--replace` and the same
`--install-dir` (if used). In a pipeline, pass arguments after `bash -s --`:

```bash
set -o pipefail
curl -fsSL https://raw.githubusercontent.com/ThomasJRyan/OnTrack/main/scripts/install.sh | bash -s -- --replace
```

The replacement is staged in the destination directory and renamed only after
download, SHA-256 verification, and extraction succeed. Restart any running
OnTrack process to use the new executable. The installer refuses a destination
symlink.

## Uninstall

Run the installed executable from any directory, including one without a Beads
workspace:

```sh
ontrack uninstall
```

The command prints the path of the running executable and the OnTrack settings
directory, then requires you to type `yes`. Use `ontrack uninstall --yes` in a
script when you have already reviewed those paths. It removes the running
`ontrack` executable, including one installed in a custom directory, and the
`ontrack` directory under `$XDG_CONFIG_HOME` or `$HOME/.config`. An absent
settings directory is fine. If either target is a symlink or has an unexpected
type or name, it stops before removal. A failure deleting settings leaves the
executable for a retry.

Beads (`bd`) and Herdr remain installed. Project-local Beads records,
screenshots, Git worktree registries, and Herdr workspaces are retained because
they may contain project history or active work. Review those within each
workspace before removing them separately. Other OnTrack processes already
running will continue until they exit.

## Troubleshooting

- **Missing `bd` or Herdr:** accept the install prompt or use the linked
  official installation guide, then confirm the command is visible in the
  same shell's `PATH`.
- **No compatible asset or release:** the latest stable GitHub Release may not
  exist yet or may not include your target. The installer exits without changing
  the destination. Check the [release page](https://github.com/ThomasJRyan/OnTrack/releases).
- **Checksum mismatch or invalid archive:** do not use the download. Retry or
  inspect the release assets; the destination remains unchanged.
- **Network or release lookup error:** check connectivity to `github.com` and
  confirm a stable release is published, then retry. Release discovery uses
  GitHub's Releases page rather than the REST API.
- **Permission error:** choose a writable directory with `--install-dir`. The
  installer does not use `sudo` or modify system directories automatically.
- **Unsupported OS, CPU, or libc:** use a supported build or build from source
  with Rust 1.88+.

## Release asset contract for maintainers

The installer uses the GitHub API's latest **stable** release for
`ThomasJRyan/OnTrack`. It recognizes the following Rust targets:

| System | Target |
| --- | --- |
| Linux x86_64 with Ubuntu 24.04-compatible glibc | `x86_64-unknown-linux-gnu` |
| Linux aarch64 with Ubuntu 24.04-compatible glibc | `aarch64-unknown-linux-gnu` |
| macOS Intel | `x86_64-apple-darwin` |
| macOS Apple silicon | `aarch64-apple-darwin` |

Publish `ontrack-vVERSION-TARGET.tar.gz` and its
`ontrack-vVERSION-TARGET.tar.gz.sha256` sidecar for every supported target.
The `vVERSION` part must equal the release's `vMAJOR.MINOR.PATCH` tag. Each
archive must contain a regular file named `ontrack` at its root; the sidecar
contains the archive's SHA-256 hash and exact filename in the standard
`HASH  FILENAME` format. The installer also checks GitHub's API asset digest
when it is present. It rejects missing assets, unexpected download URLs,
invalid checksums, and malformed archives.

Test the installer without network access with `bash tests/installer.sh`. Its
mock release exercises installation, replacement, failure handling, and
preservation of the existing executable when verification fails.
See the [release guide](releases.md) for tagging, CI gates, native builds, and
the complete published asset set.
