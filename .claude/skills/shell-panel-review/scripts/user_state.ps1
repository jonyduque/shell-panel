#Requires -Version 7.0
<#
.SYNOPSIS
  Snapshot and compare the parts of the user's machine that shell-panel's tests and installer can
  touch, so an agent can prove it left them alone.
.DESCRIPTION
  Read-only. Records, without contents:
    - HKCU\Environment Path: value kind and SHA-256 of the raw (unexpanded) value;
    - PSReadLine history file: the number of lines that look like they came from shell-panel's tests
      (the user keeps typing in other sessions, so its size and hash are not stable);
    - the real install dir (%LOCALAPPDATA%\Programs\shell-panel): file names, sizes, SHA-256;
    - the real Windows Terminal fragment dir: file names and SHA-256;
    - %USERPROFILE%\.config\shell-panel.toml and .config\shell-panel\: names and SHA-256.
  -Save <file> writes the snapshot as JSON. -Compare <file> takes a new snapshot and prints every
  difference; exit 1 when anything changed, 0 when identical. It never writes outside <file>.
  Why: an installer run with default locations once reinstalled the user's real install, and the
  installer test once leaked a test entry into the user's PATH. Both went unnoticed until reported.
.EXAMPLE
  pwsh -NoProfile -File user_state.ps1 -Save $env:TEMP\before.json
  ...run tests...
  pwsh -NoProfile -File user_state.ps1 -Compare $env:TEMP\before.json
#>
[CmdletBinding()]
param(
    [string]$Save,
    [string]$Compare
)
$ErrorActionPreference = 'Stop'

function Get-Sha([byte[]]$bytes) {
    [BitConverter]::ToString([Security.Cryptography.SHA256]::Create().ComputeHash($bytes)) -replace '-', ''
}

function Get-DirState([string]$dir) {
    if (-not (Test-Path -LiteralPath $dir)) { return [ordered]@{ exists = $false } }
    $files = [ordered]@{}
    Get-ChildItem -LiteralPath $dir -Recurse -File -Force | Sort-Object FullName | ForEach-Object {
        $files[$_.FullName.Substring($dir.Length)] = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash
    }
    [ordered]@{ exists = $true; files = $files }
}

function Get-FileState([string]$file) {
    if (-not (Test-Path -LiteralPath $file)) { return [ordered]@{ exists = $false } }
    $item = Get-Item -LiteralPath $file
    [ordered]@{ exists = $true; size = $item.Length; sha = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash }
}

function Get-TestLineCount([string]$file) {
    if (-not (Test-Path -LiteralPath $file)) { return 0 }
    # Markers the e2e tests, the PTY tests and the installer test type into sessions.
    $pattern = "'QUIET' \+ 'READY'|'WARM' \+ 'ED'|'READY' \+ 'KEY'|SP_INSTALLER_TEST|sp_e2e_|sp_report_|HELLO_SHELL_PANEL|zz_unique_file|zzalpha|SETUPDONE"
    @(Select-String -LiteralPath $file -Pattern $pattern).Count
}

function Get-State {
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment')
    try {
        $hasPath = $key.GetValueNames() -contains 'Path'
        $raw = if ($hasPath) { [string]$key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames) } else { '' }
        $path = [ordered]@{ exists = $hasPath; kind = if ($hasPath) { "$($key.GetValueKind('Path'))" } else { '' }; sha = Get-Sha ([Text.Encoding]::UTF8.GetBytes($raw)) }
    } finally { $key.Close() }

    $history = ''
    try { $history = (Get-PSReadLineOption).HistorySavePath } catch {}
    if (-not $history) { $history = Join-Path $env:APPDATA 'Microsoft\Windows\PowerShell\PSReadLine\ConsoleHost_history.txt' }

    [ordered]@{
        path      = $path
        history   = [ordered]@{ file = $history; test_lines = (Get-TestLineCount $history) }
        install   = Get-DirState (Join-Path $env:LOCALAPPDATA 'Programs\shell-panel')
        fragment  = Get-DirState (Join-Path $env:LOCALAPPDATA 'Microsoft\Windows Terminal\Fragments\shell-panel')
        config    = Get-FileState (Join-Path $env:USERPROFILE '.config\shell-panel.toml')
        specs     = Get-DirState (Join-Path $env:USERPROFILE '.config\shell-panel')
    }
}

if (-not $Save -and -not $Compare) { throw 'Pass -Save <file> or -Compare <file>.' }

$now = Get-State
if ($Save) {
    $now | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $Save -Encoding utf8
    Write-Output "[OK] user state saved to $Save (PATH sha $($now.path.sha.Substring(0,12)))"
    exit 0
}

# Hashtable key order is randomised per process in .NET, so every level above is [ordered] and the
# comparison is done section by section on JSON produced the same way on both sides.
$before = Get-Content -LiteralPath $Compare -Raw | ConvertFrom-Json
$after = $now | ConvertTo-Json -Depth 6 | ConvertFrom-Json
$changed = @()
foreach ($section in 'path', 'history', 'install', 'fragment', 'config', 'specs') {
    $x = $before.$section | ConvertTo-Json -Depth 6 -Compress
    $y = $after.$section | ConvertTo-Json -Depth 6 -Compress
    if ($x -ne $y) { $changed += $section; Write-Output "[!] $section changed`n    before: $x`n    after:  $y" }
}
if (-not $changed) {
    Write-Output '[OK] user state unchanged (PATH, history, install, Terminal fragment, config)'
    exit 0
}
Write-Output "[!] USER STATE CHANGED: $($changed -join ', '). Stop and report; do not try to repair it yourself."
exit 1
