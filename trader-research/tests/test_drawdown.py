from __future__ import annotations

import numpy as np
import pytest

from trader_research import drawdown
from trader_research.drawdown import ModoSizing


def test_drawdown_conhecido():
    # 100k -> 110k -> 88k: pico 110k, queda 22k => 20%.
    pnls = np.array([10_000.0, -22_000.0])
    assert drawdown.max_drawdown_pct(pnls, 100_000.0) == pytest.approx(20.0)


def test_serie_so_de_ganho_nao_tem_drawdown():
    assert drawdown.max_drawdown_pct(np.array([1.0, 2.0, 3.0]), 100.0) == 0.0


def test_percentual_e_sobre_o_pico_nao_sobre_o_capital():
    # Sobe 100% e cai 50k: sobre o capital daria 50%; sobre o pico, 25%.
    pnls = np.array([100_000.0, -50_000.0])
    assert drawdown.max_drawdown_pct(pnls, 100_000.0) == pytest.approx(25.0)


def test_versao_vetorizada_bate_com_a_escalar():
    rng = np.random.default_rng(5)
    lote = rng.normal(0, 500, size=(50, 30))
    esperado = np.array([drawdown.max_drawdown_pct(l, 100_000.0) for l in lote])
    obtido = drawdown._max_drawdown_pct_lote(lote, 100_000.0)
    assert np.allclose(esperado, obtido)


def test_modo_atual_reproduz_o_pnl_original():
    r = np.array([1.0, -1.0, 0.5])
    risco = np.array([100.0, 200.0, 300.0])
    mc = drawdown.monte_carlo(
        r,
        risco,
        capital=100_000.0,
        modo=ModoSizing("A", None),
        n_simulacoes=10,
        seed=1,
    )
    esperado = drawdown.max_drawdown_pct(r * risco, 100_000.0)
    assert mc.dd_observado == pytest.approx(esperado)


def test_risco_uniforme_muda_o_pnl():
    r = np.array([1.0, -1.0, 0.5])
    risco = np.array([100.0, 200.0, 300.0])
    mc = drawdown.monte_carlo(
        r,
        risco,
        capital=100_000.0,
        modo=ModoSizing("B", 0.0015),
        n_simulacoes=10,
        seed=1,
    )
    esperado = drawdown.max_drawdown_pct(r * 150.0, 100_000.0)
    assert mc.dd_observado == pytest.approx(esperado)


def test_risco_maior_produz_drawdown_maior():
    rng = np.random.default_rng(6)
    r = rng.normal(-0.05, 1.0, size=80)
    risco = np.full(80, 250.0)
    baixo = drawdown.monte_carlo(
        r, risco, capital=100_000.0, modo=ModoSizing("B", 0.0015), n_simulacoes=2000
    )
    alto = drawdown.monte_carlo(
        r, risco, capital=100_000.0, modo=ModoSizing("C", 0.0050), n_simulacoes=2000
    )
    assert alto.dd_p95 > baixo.dd_p95


def test_monte_carlo_e_deterministico():
    r = np.random.default_rng(7).normal(size=40)
    risco = np.full(40, 200.0)
    a = drawdown.monte_carlo(
        r, risco, capital=100_000.0, modo=ModoSizing("A", None), n_simulacoes=500, seed=9
    )
    b = drawdown.monte_carlo(
        r, risco, capital=100_000.0, modo=ModoSizing("A", None), n_simulacoes=500, seed=9
    )
    assert a.dd_p95 == b.dd_p95


def test_tamanhos_diferentes_sao_erro():
    with pytest.raises(ValueError):
        drawdown.monte_carlo(
            np.array([1.0]),
            np.array([1.0, 2.0]),
            capital=1.0,
            modo=ModoSizing("A", None),
        )
