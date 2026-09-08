# Modos de dimensionamento do ADR-020 (secao 6) sobre as 8 combinacoes vivas.
#
#   .\modos-sizing.ps1        -> roda os modos que faltam em out/adr020/
#   .\modos-sizing.ps1 -Force -> refaz todos
#
# O modo A e a producao (defaults do `[risk]`) e serve de REGRESSAO: os oito
# JSONs dele tem de sair identicos, trade a trade, aos wf56_* do §5.6. Se
# sairem diferentes, a ADR-020 mudou producao sem ninguem ter pedido.
#
# Os demais modos sao EXPERIMENTAIS por construcao: o `walkforward` marca
# `experimental = true` e exige `--label` sempre que uma flag de
# dimensionamento aparece, para que nenhum deles vire baseline do gate B por
# ordem de chegada (mesma trava do ADR-019 §3 e do §5.6).
param(
    [switch]$Force
)

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot\..
New-Item -ItemType Directory -Force out/adr020 | Out-Null

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

# O modo P238 emula o NOTIONAL de producao (equity ~US$ 238k) num backtest de
# 100k: sem ele o cap de liquidez quase nao morde, porque uma posicao de 100k
# ja cabe em 1/3 da barra mediana de quase todos os pares.
$modos = @(
    @{ m = "A";       r = "modo-a-status-quo";              f = @() },
    @{ m = "A3";      r = "modo-a3-fracao-1-3";             f = @("--capital-fraction", "0.3333333333") },
    @{ m = "B";       r = "modo-b-risco-015";               f = @("--risk-pct", "0.15") },
    @{ m = "Bl";      r = "modo-blinha-025-2x";             f = @("--risk-pct", "0.25", "--notional-multiple", "2") },
    @{ m = "C";       r = "modo-c-050-4x";                  f = @("--risk-pct", "0.5", "--notional-multiple", "4") },
    @{ m = "LIQ";     r = "modo-liq-um-terco";              f = @("--liquidity-pct", "33.3333333333") },
    @{ m = "P238";    r = "modo-p238-notional-de-producao"; f = @("--notional-multiple", "2.38") },
    @{ m = "P238LIQ"; r = "modo-p238-com-cap-de-liquidez";  f = @("--notional-multiple", "2.38", "--liquidity-pct", "33.3333333333") }
)

foreach ($modo in $modos) {
    foreach ($c in $combos) {
        $saida = "out/adr020/$($modo.m)_$($c.e)_$($c.s).json"
        if ((Test-Path $saida) -and -not $Force) {
            Write-Host "ja existe: $saida"
            continue
        }
        Write-Host "=== $($modo.m) $($c.e) $($c.s) ==="
        & .\target\release\trader-cli.exe walkforward `
            --symbol $c.s --strategy $c.e `
            --from 2025-02-24 --to 2026-09-03 -w 6 `
            --label $modo.r --output $saida @($modo.f) 2>&1 |
            Select-String -Pattern "Trades:|Profit factor|Avg R|Net P&L|PF em R|Max drawdown|Sizing|exportado" |
            ForEach-Object { $_.Line.Trim() }
    }
}
