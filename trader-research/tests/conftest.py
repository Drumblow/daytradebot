from __future__ import annotations

import json
from datetime import datetime, timedelta, timezone
from decimal import Decimal
from pathlib import Path
from zoneinfo import ZoneInfo

import numpy as np
import pytest

RAIZ = Path(__file__).resolve().parents[2]
# Os dois conjuntos de runs reais: `wf19_` com o custo antigo (US$ 0,35 fixos)
# e `wf56_` com a tabela real da IBKR. O crosscheck roda nos dois — ele
# confere o Python contra o `metrics.rs`, e isso vale para qualquer regua.
# `wf19_` = custo antigo (US$ 0,35 fixos), `wf56_` = tabela real da IBKR. O
# crosscheck roda nos dois: ele confere o Python contra o `metrics.rs`, e isso
# vale para qualquer regua. Outros exports do diretorio ficam de fora de
# proposito — ha runs anteriores ao ADR-019 la, sem os campos novos, e o
# crosscheck (corretamente) os recusa.
RUNS = sorted(
    (RAIZ / "out" / "adr018").glob("wf19_*.json")
) + sorted((RAIZ / "out" / "adr018").glob("wf56_*.json"))


@pytest.fixture(scope="session")
def caminhos_runs() -> list[Path]:
    if not RUNS:
        pytest.skip("sem runs em out/adr018/wf19_*.json (rode o walkforward antes)")
    return RUNS


def trade_json(
    *,
    symbol: str = "TST",
    exit_iso: str,
    net_pnl: str,
    result_in_r: str,
    risk_amount: str = "100",
    direction: str = "long",
    exit_reason: str = "target",
    commissions: str = "0.35",
    fees: str = "0",
    journal: dict | None = None,
) -> dict:
    return {
        "id": None,
        "symbol": symbol,
        "signal_id": 0,
        "position_id": None,
        "direction": direction,
        "entry_price": "10",
        "exit_price": "11",
        "quantity": "1",
        "entry_time": exit_iso,
        "exit_time": exit_iso,
        "stop_price": "9",
        "target_price": "11",
        "gross_pnl": net_pnl,
        "commissions": commissions,
        "fees": fees,
        "net_pnl": net_pnl,
        "risk_amount": risk_amount,
        "result_in_r": result_in_r,
        "exit_reason": exit_reason,
        "strategy_id": "tst",
        "strategy_version": "1.0.0",
        "config_hash": "abc",
        "journal": journal or {},
        "correlation_id": "x",
    }


def metricas_de_referencia(trades: list[dict], capital: Decimal) -> dict:
    """Implementacao INDEPENDENTE das metricas do `metrics.rs`, para fixture.

    Escrita a mao a partir da leitura do Rust, sem importar o pacote sob
    teste: e ela que faz o `crosscheck` dos testes sinteticos comparar duas
    implementacoes, e nao uma consigo mesma. Cobre so o que o crosscheck
    confere.
    """
    if not trades:
        return {}
    pnls = [Decimal(t["net_pnl"]) for t in trades]
    rs = [Decimal(t["result_in_r"]) for t in trades]
    riscos = [float(t["risk_amount"]) for t in trades]
    gp = sum((p for p in pnls if p > 0), Decimal(0))
    gl = sum((-p for p in pnls if p <= 0), Decimal(0))
    gpr = sum((r for r in rs if r > 0), Decimal(0))
    glr = sum((-r for r in rs if r <= 0), Decimal(0))
    n = len(trades)

    equity = pico = capital
    dd_abs = dd_pct = Decimal(0)
    for p in pnls:
        equity += p
        pico = max(pico, equity)
        if pico - equity > dd_abs:
            dd_abs = pico - equity
            dd_pct = Decimal(0) if pico == 0 else dd_abs / pico * Decimal(100)

    por_dia: dict = {}
    por_mes: dict = {}
    for t, p in zip(trades, pnls):
        dia = (
            datetime.fromisoformat(t["exit_time"].replace("Z", "+00:00"))
            .astimezone(ZoneInfo("America/New_York"))
            .date()
        )
        por_dia[dia] = por_dia.get(dia, Decimal(0)) + p
        por_mes[(dia.year, dia.month)] = por_mes.get((dia.year, dia.month), Decimal(0)) + p
    net = gp - gl
    net_abs = abs(net)

    def share(x: Decimal) -> float:
        return 0.0 if net_abs == 0 else float(x / net_abs)

    dias = sorted(por_dia.values(), reverse=True)
    meses = sorted(por_mes.values(), reverse=True)

    arr = np.array([float(r) for r in rs])
    if arr.size >= 2 and float(np.std(arr, ddof=1)) != 0.0:
        tstat = float(np.mean(arr)) / (float(np.std(arr, ddof=1)) / np.sqrt(arr.size))
    else:
        tstat = 0.0
    b = np.array(riscos)
    if arr.size >= 2:
        da, db = arr - arr.mean(), b - b.mean()
        den = np.sqrt((da**2).sum()) * np.sqrt((db**2).sum())
        corr = float((da * db).sum() / den) if den else 0.0
    else:
        corr = 0.0

    return {
        "total_trades": n,
        "winning_trades": sum(1 for p in pnls if p > 0),
        "losing_trades": sum(1 for p in pnls if p <= 0),
        "gross_profit": str(gp),
        "gross_loss": str(gl),
        "net_pnl": str(net),
        "profit_factor": str(gp / gl) if gl != 0 else None,
        "profit_factor_r": str(gpr / glr) if glr != 0 else None,
        "avg_r_per_trade": str(sum(rs, Decimal(0)) / n),
        "cost_total": str(
            sum((Decimal(t["commissions"]) + Decimal(t["fees"]) for t in trades), Decimal(0))
        ),
        "max_drawdown": str(dd_abs),
        "max_drawdown_pct": str(dd_pct),
        "trading_days": len(por_dia),
        "months_total": len(por_mes),
        "months_positive": sum(1 for v in por_mes.values() if v > 0),
        "top_day_share": share(dias[0]) if dias else 0.0,
        "top5_day_share": share(sum(dias[:5], Decimal(0))),
        "top2_month_share": share(sum(meses[:2], Decimal(0))),
        "t_stat_avg_r": tstat,
        "corr_risk_result": corr,
        "win_rate": str(
            Decimal(sum(1 for p in pnls if p > 0)) / Decimal(n) * Decimal(100)
        ),
        "avg_pnl_per_trade": str(net / n),
        "best_trade": str(max(pnls)),
        "worst_trade": str(min(pnls)),
        "max_consecutive_losses": _pior_sequencia(pnls),
        "by_exit_reason": _grupos(trades, _chave_saida),
        "by_direction": _grupos(trades, lambda t: t["direction"]),
        "by_entry_hour_et": _grupos(trades, _chave_hora),
    }


def _pior_sequencia(pnls) -> int:
    pior = corrente = 0
    for p in pnls:
        if p > 0:
            corrente = 0
        else:
            corrente += 1
            pior = max(pior, corrente)
    return pior


def _chave_saida(t: dict) -> str:
    diario = t.get("journal") or {}
    if t["exit_reason"] == "manual" and diario.get("forced_exit") == "session_flatten":
        return "end_of_day"
    return t["exit_reason"]


def _chave_hora(t: dict) -> str:
    momento = datetime.fromisoformat(t["entry_time"].replace("Z", "+00:00"))
    return f"{momento.astimezone(ZoneInfo('America/New_York')).hour:02d}"


def _grupos(trades: list[dict], chave) -> dict:
    """Espelha `GroupMetrics::finish` do Rust."""
    saida: dict = {}
    agrupado: dict = {}
    for t in trades:
        agrupado.setdefault(chave(t), []).append(t)
    for k, do_grupo in agrupado.items():
        pnls = [Decimal(t["net_pnl"]) for t in do_grupo]
        rs = [Decimal(t["result_in_r"]) for t in do_grupo]
        gp = sum((p for p in pnls if p > 0), Decimal(0))
        gl = sum((-p for p in pnls if p <= 0), Decimal(0))
        saida[k] = {
            "trades": len(do_grupo),
            "wins": sum(1 for p in pnls if p > 0),
            "net_pnl": str(gp - gl),
            "gross_profit": str(gp),
            "gross_loss": str(gl),
            "profit_factor": str(gp / gl) if gl != 0 else None,
            "avg_r": str(sum(rs, Decimal(0)) / len(do_grupo)),
        }
    return saida


def escreve_run(
    caminho: Path,
    trades: list[dict],
    *,
    slippage_bps: str = "2",
    session_flatten: str | None = "15:45",
    experimental: bool = False,
    label: str = "teste",
    initial_capital: str | None = "100000",
    sessions: list[str] | None = None,
    sem_calendario: bool = False,
    commission_model: str | None = "ibkr-fixed-us",
    limit_fill_haircut_bps: str | None = "2",
) -> Path:
    corpo = {
        "symbol": trades[0]["symbol"] if trades else "TST",
        "strategy_id": "tst",
        "strategy_version": "1.0.0",
        "config_hash": "abc",
        "timeframe": "M15",
        "windows": 6,
        "slippage_bps": slippage_bps,
        "session_flatten": session_flatten,
        "commission_model": commission_model,
        "limit_fill_haircut_bps": limit_fill_haircut_bps,
        "label": label,
        "experimental": experimental,
        "overrides": [],
        "strategy_source": "config/strategies/tst.toml",
        "holdout_from": None,
        "selection": {
            "windows": [],
            "oos_trades": trades,
            "oos_metrics": metricas_de_referencia(
                trades, Decimal(initial_capital or "100000")
            ),
        },
        "holdout": None,
        "holdout_trades": [],
    }
    if initial_capital is not None:
        corpo["initial_capital"] = initial_capital
    if not sem_calendario:
        # Por padrao o calendario cobre exatamente os dias com trade; os
        # testes que precisam de pregoes vazios passam `sessions`.
        corpo["oos_sessions"] = sessions or sorted(
            {
                datetime.fromisoformat(t["exit_time"].replace("Z", "+00:00"))
                .astimezone(ZoneInfo("America/New_York"))
                .date()
                .isoformat()
                for t in trades
            }
        )
    caminho.write_text(json.dumps(corpo), encoding="utf-8")
    return caminho


def serie_utc(n: int, inicio: str = "2025-03-03T18:00:00Z") -> list[str]:
    """n dias uteis consecutivos as 14h ET (18h UTC no horario de verao)."""
    base = datetime.fromisoformat(inicio.replace("Z", "+00:00")).astimezone(
        timezone.utc
    )
    saidas = []
    atual = base
    while len(saidas) < n:
        if atual.weekday() < 5:
            saidas.append(atual.isoformat().replace("+00:00", "Z"))
        atual += timedelta(days=1)
    return saidas


def calendario_util(n: int, inicio: str = "2025-03-03") -> list[str]:
    """`n` datas de dias uteis consecutivos, em ISO -- calendario sintetico."""
    from datetime import date as _date

    atual = _date.fromisoformat(inicio)
    dias = []
    while len(dias) < n:
        if atual.weekday() < 5:
            dias.append(atual.isoformat())
        atual += timedelta(days=1)
    return dias
