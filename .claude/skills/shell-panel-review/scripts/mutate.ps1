<#
.SYNOPSIS
  Prova de mutacao: troca UM trecho de codigo de producao, roda UM teste, restaura byte a byte.
.DESCRIPTION
  Exit codes (os mesmos que os relatorios devem citar):
    0 = MORTA      o teste falhou com a mutacao: a regra esta coberta.
    1 = SOBREVIVEU o teste passou com a mutacao: a regra NAO esta coberta.
    2 = INCONCLUSIVO ancora ausente/ambigua, mutacao nao compila, ou o teste nao rodou nenhum caso.
  A mutacao precisa compilar: erro de build nao e cobertura.
  Serializa mutacoes do mesmo checkout com um mutex nomeado, porque dois agentes mutando a
  mesma arvore veriam o codigo um do outro. Mesmo assim, rode em worktree: qualquer outro
  `cargo test` na mesma arvore durante a mutacao enxerga o codigo mutado.
.EXAMPLE
  pwsh -File mutate.ps1 -Path src/engine/aggregate.rs -Anchor 'text.contains(''-'')' `
       -Replacement 'text.contains(''_'')' -TestCommand 'cargo test -q --test aggregate_test'
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Path,
    [Parameter(Mandatory)][string]$Anchor,
    [Parameter(Mandatory)][AllowEmptyString()][string]$Replacement,
    [Parameter(Mandatory)][string]$TestCommand
)
$ErrorActionPreference = 'Stop'

if ($Path -match '^tests[\\/]') {
    Write-Output "[!] INCONCLUSIVO: $Path e arquivo de teste. Mutar o proprio teste nao prova cobertura; mute codigo de producao."
    exit 2
}
$full = Resolve-Path -LiteralPath $Path -ErrorAction SilentlyContinue
if (-not $full) { Write-Output "[!] INCONCLUSIVO: arquivo nao encontrado: $Path"; exit 2 }
$full = $full.Path

$bytes = [System.IO.File]::ReadAllBytes($full)
$text = [System.Text.Encoding]::UTF8.GetString($bytes)
# Ancora escrita com LF casa com arquivo CRLF e vice-versa.
$nl = if ($text.Contains("`r`n")) { "`r`n" } else { "`n" }
$anchorN = ($Anchor -replace "`r`n", "`n") -replace "`n", $nl
$replN = ($Replacement -replace "`r`n", "`n") -replace "`n", $nl

$count = ([regex]::Matches($text, [regex]::Escape($anchorN))).Count
if ($count -ne 1) {
    Write-Output "[!] INCONCLUSIVO: a ancora aparece $count vez(es) em $Path; precisa aparecer exatamente 1."
    exit 2
}
if ($anchorN -eq $replN) { Write-Output "[!] INCONCLUSIVO: replacement igual a ancora."; exit 2 }

$lineNo = ($text.Substring(0, $text.IndexOf($anchorN)) -split "`n").Count
$mutated = $text.Replace($anchorN, $replN)
$hashBefore = (Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash

$repoKey = ((Get-Location).Path.ToLowerInvariant() -replace '[^a-z0-9]', '_')
$mutex = [System.Threading.Mutex]::new($false, "Global\sp_mutate_$repoKey")
[void]$mutex.WaitOne()
$result = 2
try {
    [System.IO.File]::WriteAllText($full, $mutated, [System.Text.UTF8Encoding]::new($false))
    Write-Output "[*] mutacao em ${Path}:$lineNo"
    Write-Output "    - $Anchor"
    Write-Output "    + $Replacement"
    Write-Output "[*] $TestCommand"
    $out = & pwsh -NoProfile -Command $TestCommand 2>&1 | ForEach-Object { "$_" }
    $code = $LASTEXITCODE
    # Nao casar `error: test failed, to rerun pass ...`: essa e a falha que queremos ver.
    $compileError = $out | Where-Object { $_ -match '^error\[E\d+\]:' -or $_ -match 'could not compile' }
    $ran = $out | Where-Object { $_ -match 'test result:' -and $_ -notmatch ' 0 passed; 0 failed' }
    if ($compileError) {
        Write-Output "[!] INCONCLUSIVO: a mutacao nao compila. Erro de build nao e cobertura."
        $compileError | Select-Object -First 5 | ForEach-Object { Write-Output "    $_" }
        $result = 2
    } elseif (-not $ran) {
        Write-Output "[!] INCONCLUSIVO: nenhum teste rodou (filtro errado?). Saida:"
        $out | Select-Object -Last 10 | ForEach-Object { Write-Output "    $_" }
        $result = 2
    } elseif ($code -ne 0) {
        Write-Output "[OK] MORTA: o teste falhou com a mutacao (exit $code)."
        $out | Where-Object { $_ -match 'FAILED|panicked|failures:|^---- ' } | Select-Object -First 8 | ForEach-Object { Write-Output "    $_" }
        $result = 0
    } else {
        Write-Output "[!] SOBREVIVEU: o teste passou com a mutacao. A regra nao esta coberta por este teste."
        $out | Where-Object { $_ -match 'test result:' } | ForEach-Object { Write-Output "    $_" }
        $result = 1
    }
} finally {
    [System.IO.File]::WriteAllBytes($full, $bytes)
    $hashAfter = (Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash
    $mutex.ReleaseMutex()
    if ($hashAfter -ne $hashBefore) {
        Write-Output "[!] FALHA AO RESTAURAR $Path (hash diferente). Restaure manualmente."
        exit 3
    }
    Write-Output "[OK] restaurado byte a byte (sha256 $($hashAfter.Substring(0,12)))"
}
exit $result
