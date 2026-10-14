# Settings

OnTrack stores the selected theme globally for your Linux user, not in the
workspace or Beads database. Dark is the default; Light is also available.

## Location and format

- If `XDG_CONFIG_HOME` is nonempty and absolute:
  `$XDG_CONFIG_HOME/ontrack/settings.json`.
- Otherwise: `$HOME/.config/ontrack/settings.json` when `HOME` is absolute.
  Empty or relative `XDG_CONFIG_HOME` values are ignored.
- If neither a valid `XDG_CONFIG_HOME` nor an absolute `HOME` is available,
  resolving the settings path returns an error.

The file is a JSON object:

```json
{"theme":"dark"}
```

`theme` accepts exactly `"dark"` or `"light"`. An omitted theme uses Dark;
unknown additional keys are accepted for future expansion. Invalid JSON,
non-object JSON, unknown themes, and non-string theme values are errors.

## Changing the theme

Press `s` to open settings. The Theme dropdown starts focused. `Enter` or `Space`
opens it; Left / Right changes the choice and previews it immediately. `Enter`
keeps the current preview and closes the dropdown. `Esc` while the dropdown is
open restores the choice from when it opened; `Esc` again cancels Settings and
restores the saved theme. Use `Tab` / `Shift+Tab` to focus Save or Cancel, then
press `Enter` to activate that button. A save commits only after the write
succeeds. A failed save leaves the popup open with an error and does not change
the saved selection.

## Recovery and failure policy

A missing settings file uses defaults without creating files or directories.
On startup, a load or path-resolution error uses defaults with a visible notice;
the existing file is not silently repaired or overwritten. To recover, fix the
JSON or move/remove the file, then restart. Check directory permissions and
available disk space if saving fails.

An explicit save creates missing parent directories. It writes a uniquely named
sibling temporary file, synchronizes and closes it, then atomically renames it
over the destination. Thus readers see the previous complete file or the new
complete file, not a partial write. Concurrent successful saves are last-writer
wins. Temporary files are created with owner-only permissions on Unix. On write
or rename failure, cleanup is attempted without masking the original error;
existing settings remain untouched. Directory creation may remain after failure,
and crashes or cleanup failures can leave a sibling `.settings-<pid>-<id>.tmp` file.
Such leftovers are not loaded and can be removed while OnTrack is closed.
Atomic replacement does not guarantee survival of a sudden power loss because
the containing directory is not explicitly synchronized.

## Developer interface

`src/settings.rs` exposes `Theme`, `Settings`, `config_path()`, `load_from(path)`,
and `save_to(path, settings)`. The filesystem functions return `io::Result`;
invalid settings contents use `io::ErrorKind::InvalidData` with the file path and
reason. Loading never writes. Saving serializes the current supported fields
only, so unknown fields are accepted when reading but are not preserved on save.

The caller owns startup notices and popup state. Keep the draft separate from
the committed settings and update committed state only after `save_to` succeeds.
Path-selection tests call a private pure helper rather than mutating process
environment variables. Persistence tests use isolated temporary directories:

```sh
cargo test settings::tests
```
