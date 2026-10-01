<#
.SYNOPSIS
    Removes shell-panel installed by install.ps1.
.DESCRIPTION
    Removes what the installer wrote (shell-panel.exe, LICENSE, README.md and its marker file), the
    install folder when nothing else is left in it, the folder's entry in the user PATH and the
    Windows Terminal profile. A folder without the installer's marker is never touched. Your
    configuration (%USERPROFILE%\.config\shell-panel.toml and .config\shell-panel\) is kept unless
    -Purge is given. Windows PowerShell 5.1 and PowerShell 7.
.PARAMETER InstallDir
    Default: %LOCALAPPDATA%\Programs\shell-panel.
.PARAMETER Purge
    Also delete the configuration file and the custom specs directory.
.PARAMETER TerminalFragmentDir
    Testing only: where the Windows Terminal profile fragment was written.
#>
[CmdletBinding()]
param(
    [string]$InstallDir,
    [switch]$Purge,
    [string]$TerminalFragmentDir
)
# Same structure as install.ps1: parameters bound here, body in a child-scope script block so that
# `iex` leaves nothing behind, and @PSBoundParameters so that -Purge:$false stays false.
& {
    param(
        [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Programs\shell-panel'),
        [switch]$Purge,
        [string]$TerminalFragmentDir = (Join-Path $env:LOCALAPPDATA 'Microsoft\Windows Terminal\Fragments\shell-panel')
    )
    Set-StrictMode -Version Latest
    $ErrorActionPreference = 'Stop'
    # The PATH edit below needs .NET calls that ConstrainedLanguage refuses: fail before deleting.
    if ($ExecutionContext.SessionState.LanguageMode -ne 'FullLanguage') {
        throw "The shell-panel uninstaller needs FullLanguage mode (this session is $($ExecutionContext.SessionState.LanguageMode)). Nothing was removed."
    }
    $InstallDir = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($InstallDir)

    function Send-EnvironmentChange {
        # See install.ps1: broadcast WM_SETTINGCHANGE without rewriting Path as REG_SZ.
        [Environment]::SetEnvironmentVariable('SHELL_PANEL_INSTALLER', '1', 'User')
        [Environment]::SetEnvironmentVariable('SHELL_PANEL_INSTALLER', $null, 'User')
    }

    function Remove-UserPath([string]$dir) {
        $unwanted = $dir.TrimEnd('\')
        $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
        try {
            if ($key.GetValueNames() -contains 'Path') {
                $raw = [string]$key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
                $kind = $key.GetValueKind('Path')
                $entries = @($raw -split ';')
                $kept = @($entries | Where-Object { [Environment]::ExpandEnvironmentVariables($_).TrimEnd('\') -ine $unwanted })
                if ($kept.Count -ne $entries.Count) {
                    $key.SetValue('Path', ($kept -join ';'), $kind)
                    Send-EnvironmentChange
                }
            }
        } finally { $key.Close() }
        $env:Path = (@($env:Path -split ';' | Where-Object { $_.TrimEnd('\') -ine $unwanted }) -join ';')
    }

    $marker = Join-Path $InstallDir '.shell-panel-install'
    $exe = Join-Path $InstallDir 'shell-panel.exe'
    if (Test-Path -LiteralPath $marker) {
        try {
            if (Test-Path -LiteralPath $exe) { Remove-Item -LiteralPath $exe -Force }
        } catch {
            throw "Could not remove $exe ($($_.Exception.Message)). Close every shell-panel session and run the uninstaller again."
        }
        # Only what the installer wrote; a moved-aside binary may still be locked by a live session.
        foreach ($name in 'shell-panel.exe.old', 'LICENSE', 'README.md', '.shell-panel-install') {
            Remove-Item -LiteralPath (Join-Path $InstallDir $name) -Force -ErrorAction SilentlyContinue
        }
        if (@(Get-ChildItem -LiteralPath $InstallDir -Force).Count -eq 0) {
            Remove-Item -LiteralPath $InstallDir -Force
            Write-Host "[OK] Removed $InstallDir"
        } else {
            Write-Host "[OK] Removed shell-panel from $InstallDir (files you added there were kept)"
        }
        Remove-UserPath $InstallDir
        Write-Host '[OK] Removed from the user PATH'
    } elseif (Test-Path -LiteralPath $exe) {
        # -InstallDir is user input: a folder of other tools holding a hand-copied shell-panel.exe.
        throw "$InstallDir was not created by the shell-panel installer (no .shell-panel-install file). Nothing was removed."
    } else {
        Write-Host "[i] No shell-panel install in $InstallDir"
    }

    $fragment = Join-Path $TerminalFragmentDir 'shell-panel.json'
    if (Test-Path -LiteralPath $fragment) {
        Remove-Item -LiteralPath $fragment -Force
        if (@(Get-ChildItem -LiteralPath $TerminalFragmentDir -Force).Count -eq 0) {
            Remove-Item -LiteralPath $TerminalFragmentDir -Force
        }
        Write-Host '[OK] Removed the Windows Terminal profile'
    }

    $config = Join-Path $env:USERPROFILE '.config\shell-panel.toml'
    $specs = Join-Path $env:USERPROFILE '.config\shell-panel'
    if ($Purge) {
        foreach ($item in $config, $specs) {
            if (Test-Path -LiteralPath $item) {
                Remove-Item -LiteralPath $item -Recurse -Force
                Write-Host "[OK] Removed $item"
            }
        }
    } else {
        Write-Host "[i] Kept your configuration ($config and $specs); run with -Purge to remove it."
    }
} @PSBoundParameters
