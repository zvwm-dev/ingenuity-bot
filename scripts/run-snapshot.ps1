# ingenuity daily snapshot wrapper.
# Runs the headless snapshot binary and logs its output, so a scheduled task has a
# reliable launcher (the scheduler invokes powershell.exe, a signed system binary) and a
# durable record of each run. Read-only and rate-limited like the app.
#
# Freshness guard: if a snapshot was taken in the last 6 hours (e.g. the app was just
# refreshed, or an earlier logon already ran this), skip — so frequent logons don't hit the
# trade API more than necessary.
$ErrorActionPreference = 'Continue'
$dir = Join-Path $env:LOCALAPPDATA 'com.ingenuity.tablets'
$log = Join-Path $dir 'snapshot.log'
$exe = Join-Path $dir 'snapshot.exe'
$cache = Join-Path $dir 'valuation_Runes_of_Aldur.json'

if (Test-Path $cache) {
  $ageHours = ((Get-Date) - (Get-Item $cache).LastWriteTime).TotalHours
  if ($ageHours -lt 6) {
    "$(Get-Date -Format o)  skip (last snapshot $([int]($ageHours*60))m ago)" | Out-File -FilePath $log -Append -Encoding utf8
    exit 0
  }
}

"$(Get-Date -Format o)  start" | Out-File -FilePath $log -Append -Encoding utf8
try {
  & $exe "Runes of Aldur" 2>&1 | Out-File -FilePath $log -Append -Encoding utf8
  "$(Get-Date -Format o)  exit $LASTEXITCODE" | Out-File -FilePath $log -Append -Encoding utf8
} catch {
  "$(Get-Date -Format o)  ERROR $($_.Exception.Message)" | Out-File -FilePath $log -Append -Encoding utf8
}
