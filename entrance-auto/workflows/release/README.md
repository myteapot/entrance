# Release Workflows

These scripts are workspace automation, not product source.

- `package-release.ps1` packages a built Windows binary from
  `entrance-src/target/release/entrance.exe` into ignored
  `entrance-auto/artifacts/releases/<version>/`.
- `package-headless-alpha.ps1` wraps `package-release.ps1` with the headless
  asset naming convention.
- `export-public-snapshot.ps1` exports a filtered snapshot of `entrance-src/`.

Release notes live in `entrance-wiki/releases/<version>/RELEASE_NOTES.md`.
When version-specific notes are missing, packaging falls back to
`entrance-wiki/current/release.md`. Generated packages, zips, checksums, and
installer binaries stay under ignored artifact paths unless a human explicitly
asks to commit them.
