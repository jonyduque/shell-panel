<#
.SYNOPSIS
  Gate completo do shell-panel: formatacao, clippy sem avisos e a suite inteira.
.DESCRIPTION
  Roda no diretorio atual (checkout principal ou worktree). Saida em ASCII puro.
  Exit 0 = tudo verde; 1 = algum passo falhou (o passo e a cauda da saida sao impressos).
  Os testes PTY/e2e abrem PowerShell real via ConPTY: rode fora de outra sessao shell-panel
  ou deixe o harness remover SHELL_PANEL_SESSION (tests/common ja faz isso).
#>
[CmdletBinding()]
param(
    [switch]$SkipTests
)
$ErrorActionPreference = 'Continue'

$steps = @(
    @{ Name = 'fmt';    Cmd = 'cargo'; Args = @('fmt', '--check') },
    @{ Name = 'clippy'; Cmd = 'cargo'; Args = @('clippy', '--all-targets', '-q', '--', '-D', 'warnings') }
)
if (-not $SkipTests) {
    $steps += @{ Name = 'test'; Cmd = 'cargo'; Args = @('test', '-q') }
}

$failed = @()
foreach ($s in $steps) {
    Write-Output "[*] $($s.Name): $($s.Cmd) $($s.Args -join ' ')"
    $out = & $s.Cmd @($s.Args) 2>&1 | ForEach-Object { "$_" }
    $code = $LASTEXITCODE
    if ($code -eq 0) {
        $summary = $out | Where-Object { $_ -match '^test result:' }
        if ($summary) {
            $passed = 0; $fail = 0
            foreach ($l in $summary) {
                if ($l -match '(\d+) passed; (\d+) failed') { $passed += [int]$Matches[1]; $fail += [int]$Matches[2] }
            }
            Write-Output "[OK] $($s.Name): $($summary.Count) binarios, $passed passed, $fail failed"
        } else {
            Write-Output "[OK] $($s.Name)"
        }
    } else {
        Write-Output "[!] $($s.Name) falhou (exit $code). Ultimas 40 linhas:"
        $out | Select-Object -Last 40 | ForEach-Object { Write-Output "    $_" }
        $failed += $s.Name
    }
}

if ($failed.Count) {
    Write-Output "[!] GATE VERMELHO: $($failed -join ', ')"
    exit 1
}
Write-Output "[OK] GATE VERDE"
exit 0
