"""Concentracao temporal do P&L.

Reproduz, em Python, o que o `metrics.rs` calcula em Rust: mesmo denominador
(|net|), mesmo dia de referencia (data ET do fechamento). A duplicacao e
proposital — e ela que permite o cross-check do `crosscheck.py`. Se os dois
divergirem, um dos dois esta errado, e o teste diz qual.
"""

from __future__ import annotations

from dataclasses import dataclass
from decimal import Decimal
from typing import Sequence

from .loader import Trade
from .series import pnl_por_ano, pnl_por_dia, pnl_por_mes


@dataclass(frozen=True)
class Concentracao:
    pregoes: int
    top_dia: float
    top5_dias: float
    top2_meses: float
    meses: int
    meses_positivos: int
    por_ano: dict[int, Decimal]
    wr_diario: float
    dias_positivos: int


def concentracao(trades: Sequence[Trade]) -> Concentracao:
    dias = pnl_por_dia(trades)
    meses = pnl_por_mes(trades)
    net = sum(dias.values(), Decimal(0))
    net_abs = abs(net)

    def share(soma: Decimal) -> float:
        return 0.0 if net_abs == 0 else float(soma / net_abs)

    valores_dia = sorted(dias.values(), reverse=True)
    valores_mes = sorted(meses.values(), reverse=True)
    positivos = sum(1 for v in dias.values() if v > 0)
    return Concentracao(
        pregoes=len(dias),
        top_dia=share(valores_dia[0]) if valores_dia else 0.0,
        top5_dias=share(sum(valores_dia[:5], Decimal(0))),
        top2_meses=share(sum(valores_mes[:2], Decimal(0))),
        meses=len(meses),
        meses_positivos=sum(1 for v in meses.values() if v > 0),
        por_ano=pnl_por_ano(trades),
        # Item 9 da lista de pendencias do ADR-019: win rate por DIA, que o
        # `metrics.rs` nao calcula (ele so conta pregoes).
        wr_diario=(positivos / len(dias) * 100.0) if dias else 0.0,
        dias_positivos=positivos,
    )
