# shell-panel integration for PowerShell with PSReadLine 2.x. Passed with -EncodedCommand.
# Messages to shell-panel: ESC ] 6973;<payload> BEL with payloads RS;<cwd>, RE and CMP;<json>.

function Global:__SP-Escape([string]$value) {
    [regex]::Replace($value, '[\x00-\x1f\x7f\\;]', { param($match)
        -join ([System.Text.Encoding]::UTF8.GetBytes($match.Value) | ForEach-Object { '\x{0:x2}' -f $_ })
    })
}

function Global:__SP-Send([string]$payload) {
    [Console]::Write("$([char]0x1b)]6973;$payload$([char]0x07)")
}

# Mark the time PSReadLine spends reading a line. Unlike a prompt wrapper this survives the user
# redefining `prompt` and does not change what the prompt function sees.
if (Get-Command PSConsoleHostReadLine -ErrorAction Ignore) {
    # Capturing our own wrapper on a second load would recurse until the stack overflows.
    if (-not $Global:__SP_OriginalReadLine) {
        $Global:__SP_OriginalReadLine = $function:PSConsoleHostReadLine
    }
    function Global:PSConsoleHostReadLine {
        $cwd = if ($pwd.Provider.Name -eq 'FileSystem') { $pwd.ProviderPath } else { '' }
        __SP-Send "RS;$(__SP-Escape $cwd)"
        try { $Global:__SP_OriginalReadLine.Invoke() } finally { __SP-Send 'RE' }
    }
}

# PSReadLine's prediction list covers the rows shell-panel draws its dropdown on, so the two
# collide. Only the list view is in the way: an inline prediction and the prediction source itself
# are left as the user configured them. Older PSReadLine versions have no such option.
try {
    if ((Get-PSReadLineOption).PredictionViewStyle -eq 'ListView') {
        Set-PSReadLineOption -PredictionViewStyle InlineView
    }
} catch {}

# Ctrl+Alt+Shift+F12 (sent by shell-panel when Tab is pressed): report line, cursor and completions.
try {
    Set-PSReadLineKeyHandler -Chord 'Ctrl+Alt+Shift+F12' -BriefDescription 'ShellPanelReport' -ScriptBlock {
        # A failure here must never print over the line the user is editing.
        try {
            $line = $null
            $cursor = $null
            [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref]$line, [ref]$cursor)
            $report = @{ line = [string]$line; cursor = $cursor; replacementIndex = $cursor; replacementLength = 0; matches = @() }
            try {
                $completion = [System.Management.Automation.CommandCompletion]::CompleteInput($line, $cursor, $null)
                $report.replacementIndex = $completion.ReplacementIndex
                $report.replacementLength = $completion.ReplacementLength
                $report.matches = @($completion.CompletionMatches | Select-Object -First 100 | ForEach-Object {
                    $tip = "$($_.ToolTip)"
                    if ($tip.Length -gt 120) {
                        $cut = if ([char]::IsHighSurrogate($tip[119])) { 119 } else { 120 }
                        $tip = $tip.Substring(0, $cut)
                    }
                    , @($_.CompletionText, $_.ListItemText, $_.ResultType.ToString(), $tip)
                })
            } catch {}
            __SP-Send "CMP;$(__SP-Escape (ConvertTo-Json -InputObject $report -Compress -Depth 4))"
        } catch {}
    }
} catch {}
