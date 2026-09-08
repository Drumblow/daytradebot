"""Agregacao de trades em series diarias e mensais.

O dia de referencia e a data de **Nova York do fechamento** -- a mesma regra
de `crates/trader-backtest/src/metrics.rs` (`et_date(trade.exit_time)`).

Uma versao anterior deste texto justificava a regra com um exemplo errado
("um fechamento as 16h05 ET de dezembro cai no dia seguinte em UTC"): 16h05
EST e 21h05 UTC, o MESMO dia civil. A data UTC so passa a divergir da ET a
partir das 19h ET no inverno (20h ET no verao) -- ou seja, com posicao
mantida depois do pregao, o que a regua atual (flatten as 15h45, ADR-018)
impede. Com os dados de hoje a conversao nao muda numero nenhum.

Ela continua sendo a regra certa por dois motivos que NAO dependem disso:
paridade com o motor, que agrupa por data ET; e o dia em que houver trade
overnight (`hold_overnight` do plano secao 5.7, v2 com short) ou saida em
sessao estendida, quando a divergencia aparece sem aviso. O A2 de verdade e o
caso irmao do `by_entry_hour_et`, onde o balde de hora em UTC muda de
significado na virada do DST e junta duas horas de pregao diferentes.
"""

from __future__ import annotations

from datetime import date
from decimal import Decimal
from typing import Sequence
from zoneinfo import ZoneInfo

import numpy as np

from .loader import Trade

NY = ZoneInfo("America/New_York")


def et_date(momento) -> date:
    return momento.astimezone(NY).date()


def pnl_por_dia(trades: Sequence[Trade]) -> dict[date, Decimal]:
    """P&L liquido por pregao ATIVO (dias sem trade nao aparecem)."""
    por_dia: dict[date, Decimal] = {}
    for t in trades:
        dia = et_date(t.exit_time)
        por_dia[dia] = por_dia.get(dia, Decimal(0)) + t.net_pnl
    return dict(sorted(por_dia.items()))


def r_por_dia(trades: Sequence[Trade]) -> dict[date, Decimal]:
    por_dia: dict[date, Decimal] = {}
    for t in trades:
        dia = et_date(t.exit_time)
        por_dia[dia] = por_dia.get(dia, Decimal(0)) + t.result_in_r
    return dict(sorted(por_dia.items()))


def pnl_por_mes(trades: Sequence[Trade]) -> dict[tuple[int, int], Decimal]:
    por_mes: dict[tuple[int, int], Decimal] = {}
    for t in trades:
        dia = et_date(t.exit_time)
        chave = (dia.year, dia.month)
        por_mes[chave] = por_mes.get(chave, Decimal(0)) + t.net_pnl
    return dict(sorted(por_mes.items()))


def pnl_por_ano(trades: Sequence[Trade]) -> dict[int, Decimal]:
    """P&L por ano civil — item 7 da lista de pendencias do ADR-019."""
    por_ano: dict[int, Decimal] = {}
    for t in trades:
        ano = et_date(t.exit_time).year
        por_ano[ano] = por_ano.get(ano, Decimal(0)) + t.net_pnl
    return dict(sorted(por_ano.items()))


def vetor_diario(trades: Sequence[Trade]) -> np.ndarray:
    """Serie de P&L diario como float64, em ordem cronologica.

    f64 e permitido aqui (ADR-019 secao 8): a regra do AGENTS.md proibe f64
    para **dinheiro** — valor que vira ordem ou posicao. Nada deste modulo
    volta para o motor como dinheiro.
    """
    return np.array([float(v) for v in pnl_por_dia(trades).values()], dtype=np.float64)


def vetor_r(trades: Sequence[Trade]) -> np.ndarray:
    return np.array([float(t.result_in_r) for t in trades], dtype=np.float64)


def vetor_risco(trades: Sequence[Trade]) -> np.ndarray:
    return np.array([float(t.risk_amount) for t in trades], dtype=np.float64)
