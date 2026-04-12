# Entrance v1.0.0-rc.1

## Highlights

- Native desktop shell with Chat, Dashboard, Forge, Vault, Settings, Console, and built-in Issues surfaces.
- Built-in issue tracker now spans GUI, CLI (`entrance issues`), and MCP issue CRUD tools.
- Forge dispatch is DB-first and no longer depends on live Linear status lookups to prepare work.
- Carbon visual system is now the default UI language across the application.

## Operator Notes

- Runtime truth remains anchored in `~/.entrance/data/entrance.db`.
- Updater metadata for this release candidate is staged in GitLab, but signed platform artifacts still need to be attached before public updater pickup should be enabled.
- Release packaging should use `releases/package-release.ps1` after building `hosts/desktop/tauri\\target\\release\\entrance.exe`.
