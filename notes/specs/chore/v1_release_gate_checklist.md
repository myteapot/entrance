# V1 Release Gate Checklist

> Scope: V1 self-consistency + Linux/Windows/Electron validation

## Quick Run

Linux/macOS:

```bash
./hosts/release/verify-v1-self-consistency.sh
```

Windows PowerShell:

```powershell
./hosts/release/verify-v1-self-consistency.ps1
```

Electron host smoke:

```bash
pnpm test:electron-smoke
```

Windows native smoke runner:

```powershell
./hosts/release/run-windows-native-smoke.ps1 -Configuration Release
```

## Gate Conditions

- Runtime closure:
  - `round_state.state == fully_settled`
  - `round_state.carry_forward_checkpointed == true`
  - `nota invariants.failed_count == 0`
  - `nota repair.open_count == 0`
- Landing reconciliation:
  - run batch-01 manifest (`12` items)
  - `MYT-56/61/63/64/65` are not `unreconciled`
  - `landing reconcile report.unreconciled_count <= 38`
- Baseline checks:
  - `cargo test --manifest-path hosts/desktop/tauri/Cargo.toml --lib`
  - `pnpm check`
  - `pnpm test:e2e` (Linux rollup native preflight included)
- Dual-host acceptance:
  - Electron smoke passes (`invoke/listen` + route sweep)
  - Windows native smoke passes (`dotnet test hosts/platform/windows/tests/native/EntranceNativeTests.csproj`)
  - GitLab pipeline jobs are all green: `linux-verify`, `electron-smoke`, `windows-native`

## Manual Fallbacks

- Skip e2e when triaging non-browser failures:

```bash
./hosts/release/verify-v1-self-consistency.sh --skip-e2e
```

```powershell
./hosts/release/verify-v1-self-consistency.ps1 -SkipE2E
```

- Use a prebuilt Entrance binary instead of `cargo run`:

```bash
ENTRANCE_EXE_PATH=/path/to/entrance ./hosts/release/verify-v1-self-consistency.sh
```

```powershell
$env:ENTRANCE_EXE_PATH = "C:\path\to\entrance.exe"
./hosts/release/verify-v1-self-consistency.ps1
```

- Run Windows native smoke against a prebuilt exe:

```powershell
./hosts/release/run-windows-native-smoke.ps1 `
  -SkipBuild `
  -EntranceExePath "C:\path\to\entrance.exe" `
  -Configuration Release
```
