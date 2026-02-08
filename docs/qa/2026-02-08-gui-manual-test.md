# GUI Manual Test Checklist (2026-02-08)

## Scope
- tonsuu-gui basic navigation and core workflows
- Settings, Vehicles, History, Accuracy tabs

## Preconditions
- Build succeeds: `cargo test`
- Optional: demo data exists in history/vehicles stores

## Launch
1. Run `cargo run -p tonsuu-gui`
2. Confirm app opens without crash

## Tab Navigation
1. Click `解析`
2. Click `車両`
3. Click `履歴`
4. Click `精度`
5. Click `設定`

## 解析 (Analyze)
1. Select image and run analysis
2. Confirm progress and result output
3. If cache enabled, re-run and confirm cache hit behavior

## 車両 (Vehicle)
1. Add a vehicle and save
2. Edit existing vehicle and save
3. Delete a vehicle and confirm removal
4. Verify filter/search if available

## 履歴 (History)
1. Confirm entries list loads
2. Open context menu and re-analyze
3. Verify update appears

## 精度 (Accuracy)
1. Check overall statistics show
2. Verify group-by options if present

## 設定 (Settings)
1. Change backend selection
2. Change usage mode
3. Set model value
4. Save settings and relaunch to confirm persistence

## Data Import (if available)
1. Open legacy import dialog
2. Load a backup JSON
3. Preview summary
4. Run import (dry-run or append)

## Pass Criteria
- No crashes
- Tabs render and basic workflows complete
- Settings persist across restart
