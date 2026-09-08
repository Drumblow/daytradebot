"""Monte Carlo de drawdown reamostrando (R, risco) em conjunto.

Por que reamostrar o PAR e nao o P&L: no motor, `net_pnl = result_in_r *
risk_amount` exatamente. Reamostrar o P&L pronto responde "que drawdown esta
sequencia teria noutra ordem"; reamostrar o par permite trocar o *sizing* sem
tocar no resultado da estrategia — que e a pergunta do ADR-020 (secao 5.5 do
plano): o R e propriedade da regra, o risco em dolares e politica de risco.

Limite declarado: os modos de fracao fixa abaixo IGNORAM o teto de notional,
que hoje e o que realmente prende (o plano, secao 5.5, mostra que a 0,25% o
cap morde para stop < 25 bp). Sem o cap, esses modos superestimam o risco
tomado — o drawdown deles e um teto, nao uma previsao.
"""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np


def max_drawdown_pct(pnls: np.ndarray, capital: float) -> float:
    """Drawdown maximo em % do pico, com a equity comecando em `capital`.

    Mesma convencao do `metrics.rs`: `peak` e `current_equity` comecam no
    capital inicial e o percentual e sobre o PICO, nao sobre o capital.
    """
    equity = capital + np.cumsum(pnls)
    picos = np.maximum.accumulate(np.concatenate(([capital], equity)))[1:]
    quedas = np.where(picos > 0, (picos - equity) / picos, 0.0)
    return float(np.max(np.concatenate(([0.0], quedas))) * 100.0)


def _max_drawdown_pct_lote(pnls: np.ndarray, capital: float) -> np.ndarray:
    """Versao vetorizada sobre uma matriz (n_sim, n_trades)."""
    equity = capital + np.cumsum(pnls, axis=1)
    picos = np.maximum.accumulate(
        np.concatenate((np.full((pnls.shape[0], 1), capital), equity), axis=1), axis=1
    )[:, 1:]
    quedas = np.where(picos > 0, (picos - equity) / picos, 0.0)
    return quedas.max(axis=1) * 100.0


@dataclass(frozen=True)
class ModoSizing:
    """Como o risco em dolares de cada trade e recalculado.

    `fracao` None mantem o risco original do run (modo A do plano). Um valor
    substitui o risco por `fracao * capital` em todo trade (risco uniforme).
    `alavancagem` fica registrada porque muda o que e *executavel*, nao o
    risco: sem ela, o cap de notional prende antes.
    """

    nome: str
    fracao: float | None
    alavancagem: float = 1.0
    nota: str = ""


MODOS_PADRAO = [
    ModoSizing("A · atual", None, 1.0, "o risco de cada trade como o motor calculou"),
    ModoSizing("B · 0,15% / 1x", 0.0015, 1.0, "risco uniforme, sem alavancagem"),
    ModoSizing("B' · 0,25% / 2x", 0.0025, 2.0, "quase o status quo quando o cap nao prende"),
    ModoSizing("C · 0,50% / 4x", 0.0050, 4.0, "exige alavancagem que o teto de 200% bloqueia"),
]


@dataclass(frozen=True)
class ResultadoMC:
    modo: str
    n_simulacoes: int
    dd_mediano: float
    dd_p95: float
    dd_p99: float
    dd_observado: float
    prob_acima_de_10pct: float
    nota: str


def monte_carlo(
    r: np.ndarray,
    risco: np.ndarray,
    *,
    capital: float,
    modo: ModoSizing,
    n_simulacoes: int = 10_000,
    seed: int = 20260907,
) -> ResultadoMC:
    """Reamostra os pares (R, risco) com reposicao e mede o drawdown."""
    if r.size != risco.size:
        raise ValueError("R e risco precisam ter o mesmo tamanho")
    if r.size == 0:
        raise ValueError("amostra vazia")
    risco_efetivo = (
        risco if modo.fracao is None else np.full_like(risco, modo.fracao * capital)
    )
    pnl = r * risco_efetivo
    rng = np.random.default_rng(seed)
    idx = rng.integers(0, r.size, size=(n_simulacoes, r.size))
    dds = _max_drawdown_pct_lote(pnl[idx], capital)
    return ResultadoMC(
        modo=modo.nome,
        n_simulacoes=n_simulacoes,
        dd_mediano=float(np.percentile(dds, 50)),
        dd_p95=float(np.percentile(dds, 95)),
        dd_p99=float(np.percentile(dds, 99)),
        dd_observado=max_drawdown_pct(pnl, capital),
        prob_acima_de_10pct=float(np.mean(dds > 10.0)),
        nota=modo.nota,
    )
