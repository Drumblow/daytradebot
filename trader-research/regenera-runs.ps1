# Regera os 8 walk-forwards do gate A das combinacoes vivas.
#
#   .\regenera-runs.ps1            -> custo real da IBKR (secao 5.6),
#                                     out/adr018/wf56_*.json
#   .\regenera-runs.ps1 -Legacy    -> custo antigo (US$ 0,35 fixos por perna e
#                                     alvo limite de graca),
#                                     out/adr018/wf19_*.json
#
# Os dois conjuntos NAO sao somaveis: o `trader-research` recusa juntar runs
# com modelos de comissao diferentes, pelo mesmo motivo que recusa juntar com
# e sem flatten. O modo -Legacy existe para reproduzir os numeros publicados
# ate 08/09/2026 e medir o delta.
param(
    [switch]$Legacy
)

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot\..

if ($Legacy) {
    $prefixo = "wf19"
    $rotulo = "gate-a-adr019"
    $extra = @("--legacy-cost")
} else {
    $prefixo = "wf56"
    $rotulo = "gate-a-custo-real"
    $extra = @()
}

$combos = @(
    @{ e = "balance-area-breakout-v1"; s = "IJS" },
    @{ e = "balance-area-breakout-v1"; s = "VBR" },
    @{ e = "balance-area-breakout-v1"; s = "AVUV" },
    @{ e = "opening-reversal-v1";      s = "IWM" },
    @{ e = "opening-reversal-v1";      s = "IWN" },
    @{ e = "range-extreme-fade-v1";    s = "AVUV" },
    @{ e = "range-extreme-fade-v1";    s = "SLYV" },
    @{ e = "range-extreme-fade-v1";    s = "IWV" }
)

foreach ($c in $combos) {
    $saida = "out/adr018/${prefixo}_$($c.e)_$($c.s).json"
    Write-Host "=== $($c.e) $($c.s) ==="
    & .\target\release\trader-cli.exe walkforward `
        --symbol $c.s --strategy $c.e `
        --from 2025-02-24 --to 2026-09-03 -w 6 `
        --label $rotulo --output $saida @extra 2>&1 |
        Select-String -Pattern "Trades:|Profit factor|Avg R|Net P&L|PF em R|2 melhores|custo:|exportado" |
        ForEach-Object { $_.Line.Trim() }
}
