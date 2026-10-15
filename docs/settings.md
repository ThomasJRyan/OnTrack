# Settings

OnTrack stores the selected theme globally for your Linux user, not in the
workspace or Beads database. OnTrack Dark is the default. Twelve built-in themes
are available:

| Stable ID | Display name | Style |
| --- | --- | --- |
| `dark` | OnTrack Dark | Dark |
| `light` | OnTrack Light | Light |
| `tokyo-night` | Tokyo Night | Dark |
| `nord` | Nord | Dark |
| `dracula` | Dracula | Dark |
| `gruvbox-dark` | Gruvbox Dark | Dark |
| `gruvbox-light` | Gruvbox Light | Light |
| `catppuccin-mocha` | Catppuccin Mocha | Dark |
| `catppuccin-latte` | Catppuccin Latte | Light |
| `solarized-dark` | Solarized Dark | Dark |
| `solarized-light` | Solarized Light | Light |
| `rose-pine` | Rosé Pine | Dark |

## Palette references and attribution

Theme names identify visual inspirations; OnTrack assigns semantic roles,
contrast adjustments, and interface colors for this terminal UI. OnTrack is not
affiliated with these projects. See their upstream references and licenses:

- Theme-catalog inspiration: [OpenCode v2 themes](https://opencode.ai/v2/docs/themes).
- [Tokyo Night](https://github.com/folke/tokyonight.nvim) — Apache-2.0
  ([license](https://github.com/folke/tokyonight.nvim/blob/main/LICENSE)).
- [Gruvbox](https://github.com/morhetz/gruvbox) — MIT
  ([license](https://github.com/morhetz/gruvbox/blob/master/LICENSE)).
- [Nord](https://nordtheme.com/docs/colors-and-palettes).
- [Dracula](https://draculatheme.com/spec).
- [Catppuccin](https://catppuccin.com/palette) — MIT
  ([license](https://github.com/catppuccin/catppuccin/blob/main/LICENSE)).
- [Solarized](https://ethanschoonover.com/solarized/).
- [Rosé Pine](https://github.com/rose-pine/rose-pine).

These references are attribution, not an affiliation claim. Any exact upstream
palette data reused or adapted must keep its required copyright and license
notices with the redistributed data; consult the current upstream license.
The OnTrack-specific role mapping and contrast choices are maintained in
`src/theme.rs`.

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

`theme` accepts one of the stable IDs above; existing `"dark"` and `"light"`
files remain valid. An omitted theme uses OnTrack Dark; unknown additional keys
are accepted for future expansion. Invalid JSON,
non-object JSON, unknown themes, and non-string theme values are errors.

## Changing the theme

Press `s` to open settings. The Theme dropdown starts focused. `Enter` or `Space`
opens it; Left / Right or Up / Down cycles through every theme with wraparound
and previews the selection immediately. The bounded list scrolls to keep the
current choice visible. `Enter` keeps the preview and closes the list. `Esc`
while the list is open restores its opening choice; `Esc` again cancels Settings and
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
To add a theme, add a stable variant to `settings::Theme`, then add exactly one
`ThemeDefinition` to `theme::CATALOG` with a unique kebab-case ID, display name,
light/dark classification, and complete palette. Keep `Theme::ALL` in catalog
order, retain the existing `dark` and `light` IDs, and extend catalog,
round-trip, contrast, and renderer coverage. Use palette roles in `ui.rs` instead
of adding theme-specific conditions there.
Path-selection tests call a private pure helper rather than mutating process
environment variables. Persistence tests use isolated temporary directories:

```sh
cargo test settings::tests
```
