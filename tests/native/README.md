# Native Smoke Contract (Windows)

`SmokeTest.cs` launches a built `entrance.exe` and asserts the main window appears with an `Entrance` title.

## Contract

- Input binary path comes from `ENTRANCE_EXE_PATH`.
- If `ENTRANCE_EXE_PATH` is not set, fallback path is:
  - `src-tauri/target/release/entrance.exe` (via relative test project traversal).

## Recommended Runner

Use the release runner script so build + env wiring stay reproducible:

```powershell
./scripts/release/run-windows-native-smoke.ps1 -Configuration Release
```

Use a prebuilt binary:

```powershell
./scripts/release/run-windows-native-smoke.ps1 `
  -SkipBuild `
  -EntranceExePath "C:\path\to\entrance.exe" `
  -Configuration Release
```
