# tonsuu-checker

Dump truck payload estimation toolset (CLI/GUI) organized as a Cargo workspace.

## Workspace Layout

- `crates/tonsuu-app` Application layer (use cases, config, repository adapters)
- `crates/tonsuu-cli` CLI entrypoint and commands
- `crates/tonsuu-domain` Domain models and services
- `crates/tonsuu-gui` GUI application
- `crates/tonsuu-infra` IO, persistence, CSV, legacy import
- `crates/tonsuu-store` History/vehicle stores
- `crates/tonsuu-types` Shared types and errors
- `crates/tonsuu-vision` AI/prompt/analysis helpers

## Build

```powershell
cargo build --release
```

## Test

```powershell
cargo test --workspace
```

### Ground Truth (CLI)

Run a single sample by index (1-based) or by number/rank using env vars:

```powershell
$env:TONSUU_GT_INDEX="1"; cargo test -p tonsuu-cli --test ground_truth_test -- --nocapture
$env:TONSUU_GT_NUMBER="1122"; cargo test -p tonsuu-cli --test ground_truth_test -- --nocapture
$env:TONSUU_GT_RANK="low"; cargo test -p tonsuu-cli --test ground_truth_test -- --nocapture
$env:TONSUU_GT_ALL="1"; cargo test -p tonsuu-cli --test ground_truth_test -- --nocapture
$env:TONSUU_GT_INDEX="1"; $env:TONSUU_GT_HEIGHT_ONLY="1"; cargo test -p tonsuu-cli --test ground_truth_test -- --nocapture
```
