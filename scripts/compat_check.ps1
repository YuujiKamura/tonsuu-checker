param(
  [string]$StoreDir = "",
  [string]$SlipsCsv = "",
  [string]$VehiclesCsv = "",
  [string]$Config = "",
  [string]$Jsonl = "",
  [string]$Json = ""
)

$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$repo = Split-Path -Parent $root

$cmd = @("cargo", "run", "-p", "tonsuu-cli", "--bin", "compat_check", "--")

if ($Config -ne "") {
  $cmd += "--config"
  $cmd += $Config
}

if ($StoreDir -ne "") {
  $cmd += "--store-dir"
  $cmd += $StoreDir
}

if ($SlipsCsv -ne "" -and $VehiclesCsv -ne "") {
  $cmd += "--slips-csv"
  $cmd += $SlipsCsv
  $cmd += "--vehicles-csv"
  $cmd += $VehiclesCsv
}

if ($Jsonl -ne "") {
  $cmd += "--jsonl"
  $cmd += $Jsonl
}

if ($Json -ne "") {
  $cmd += "--json"
  $cmd += $Json
}

Push-Location $repo
try {
  & $cmd[0] $cmd[1..($cmd.Length-1)]
} finally {
  Pop-Location
}
