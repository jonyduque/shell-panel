<#
.SYNOPSIS
  Confere, sem agente, que cada achado cita um trecho que existe no arquivo, perto da linha citada.
.DESCRIPTION
  Entrada: JSON com uma lista de achados (ou objeto com a propriedade 'achados'), cada um com
  'arquivo', 'linha' e 'trecho'. Compara com espacos normalizados; trecho com varias linhas
  e procurado linha a linha (todas precisam existir, em ordem).
  Saida por achado: OK | DESLOCADA (existe, mas a mais de -Tolerance linhas) | MORTA (nao existe)
  | SEM-ARQUIVO. Exit 1 quando ha MORTA ou SEM-ARQUIVO.
  Ancora morta e o defeito mais comum de relatorio de revisao e parece achado real.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$FindingsJson,
    [int]$Tolerance = 15
)
function Norm([string]$s) { ($s -replace '\s+', ' ').Trim() }

$data = Get-Content -LiteralPath $FindingsJson -Raw -Encoding utf8 | ConvertFrom-Json
$items = if ($data.PSObject.Properties.Name -contains 'achados') { $data.achados } else { $data }
$bad = 0; $i = 0
foreach ($a in $items) {
    $i++
    $tag = "#$i $($a.arquivo):$($a.linha)"
    if (-not $a.trecho) { Write-Output "[!] SEM-TRECHO $tag"; $bad++; continue }
    if (-not (Test-Path -LiteralPath $a.arquivo)) { Write-Output "[!] SEM-ARQUIVO $tag"; $bad++; continue }
    $lines = Get-Content -LiteralPath $a.arquivo -Encoding utf8
    $want = @($a.trecho -split "`r?`n" | ForEach-Object { Norm $_ } | Where-Object { $_ -ne '' -and $_ -ne '...' })
    $first = $null; $pos = 0; $ok = $true
    foreach ($w in $want) {
        $found = $null
        for ($k = $pos; $k -lt $lines.Count; $k++) {
            if ((Norm $lines[$k]).Contains($w)) { $found = $k; break }
        }
        if ($null -eq $found) { $ok = $false; break }
        if ($null -eq $first) { $first = $found }
        $pos = $found + 1
    }
    if (-not $ok) { Write-Output "[!] MORTA $tag  trecho: $($want[0])"; $bad++; continue }
    $delta = [math]::Abs(($first + 1) - [int]$a.linha)
    if ($delta -gt $Tolerance) { Write-Output "[i] DESLOCADA $tag  (trecho na linha $($first + 1))" }
    else { Write-Output "[OK] $tag" }
}
Write-Output "[i] $i achado(s), $bad com ancora morta"
if ($bad) { exit 1 }
exit 0
