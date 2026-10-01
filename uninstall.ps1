<#
.SYNOPSIS
    Removes shell-panel installed by install.ps1.
.DESCRIPTION
    Removes the install directory, its entry in the user PATH and the Windows Terminal profile.
    Your configuration (%USERPROFILE%\.config\shell-panel.toml and .config\shell-panel\) is kept
    unless -Purge is given. Windows PowerShell 5.1 and PowerShell 7.
.PARAMETER InstallDir
    Default: %LOCALAPPDATA%\Programs\shell-panel.
.PARAMETER Purge
    Also delete the configuration file and the custom specs directory.
.PARAMETER TerminalFragmentDir
    Testing only: where the Windows Terminal profile fragment was written.
#>
# Same structure as install.ps1: a script block in a child scope, so `iex` leaves nothing behind.
& {
    [CmdletBinding()]
    param(
        [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Programs\shell-panel'),
        [switch]$Purge,
        [string]$TerminalFragmentDir = (Join-Path $env:LOCALAPPDATA 'Microsoft\Windows Terminal\Fragments\shell-panel')
    )
    Set-StrictMode -Version Latest
    $ErrorActionPreference = 'Stop'

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

    if (Test-Path -LiteralPath $InstallDir) {
        # Never delete a directory that is not a shell-panel install: -InstallDir is user input.
        if (-not (Test-Path -LiteralPath (Join-Path $InstallDir 'shell-panel.exe'))) {
            throw "$InstallDir does not contain shell-panel.exe; it does not look like a shell-panel install. Nothing was removed."
        }
        try {
            Remove-Item -LiteralPath $InstallDir -Recurse -Force
        } catch {
            throw "Could not remove $InstallDir ($($_.Exception.Message)). Close every shell-panel session and run the uninstaller again."
        }
        Write-Host "[OK] Removed $InstallDir"
    } else {
        Write-Host "[i] $InstallDir does not exist"
    }

    Remove-UserPath $InstallDir
    Write-Host '[OK] Removed from the user PATH'

    if (Test-Path -LiteralPath $TerminalFragmentDir) {
        Remove-Item -LiteralPath $TerminalFragmentDir -Recurse -Force
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
} @args
