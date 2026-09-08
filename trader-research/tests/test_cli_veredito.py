"""A linha que o dono vai ler para decidir.

O veredito do criterio proposto sai de UMA comparacao (`ic_l5.inferior >=
1.0`). A revisao adversarial de 08/09/2026 mostrou que trocar `.inferior` por
`.superior`, ou pegar o IC iid no lugar do de blocos, invertia o texto
impresso com a suite 100% verde: nao havia nenhum teste de `cli.py`.
"""

from __future__ import annotations

import re

import pytest

from conftest import calendario_util, escreve_run, serie_utc, trade_json
from trader_research import bootstrap as bs
from trader_research.cli import Unidade, monta_relatorio
from trader_research.loader import load_run

CALENDARIO = calendario_util(150)
DIAS = serie_utc(150)


def _run(tmp_path, nome, pnls, symbol="AAA"):
    trades = [
        trade_json(symbol=symbol, exit_iso=d, net_pnl=str(p), result_in_r=str(p / 100.0))
        for d, p in zip(DIAS, pnls)
    ]
    return load_run(escreve_run(tmp_path / nome, trades, sessions=CALENDARIO))


def _inferior_impresso(texto: str) -> float:
    m = re.search(r"IC95 do PF em blocos \(L=5\) = ([-\d.]+)", texto)
    assert m, texto
    return float(m.group(1))


def _run_bom(tmp_path):
    # 40 pregoes com trade em 150: ganhos frequentes e perdas pequenas, para
    # que o limite inferior do IC fique acima de 1 com folga.
    pnls = [200.0 if i % 4 else -30.0 for i in range(40)]
    return _run(tmp_path, "bom.json", pnls)


def _run_ruim(tmp_path):
    pnls = [40.0 if i % 4 else -300.0 for i in range(40)]
    return _run(tmp_path, "ruim.json", pnls)


def test_serie_com_edge_imprime_PASSA(tmp_path):
    texto = monta_relatorio([_run_bom(tmp_path)], "par", [2, 6], 4000)
    assert "**PASSA**" in texto
    assert "**NAO passa**" not in texto
    assert _inferior_impresso(texto) > 1.0


def test_serie_sem_edge_imprime_NAO_passa(tmp_path):
    texto = monta_relatorio([_run_ruim(tmp_path)], "par", [2, 6], 4000)
    assert "**NAO passa**" in texto
    assert "**PASSA**" not in texto
    assert _inferior_impresso(texto) < 1.0


def test_o_numero_impresso_e_o_limite_INFERIOR_do_bloco_de_5(tmp_path):
    # Prende as duas trocas plausiveis de uma vez: `.superior` no lugar de
    # `.inferior`, e o IC iid no lugar do de blocos.
    run = _run_bom(tmp_path)
    texto = monta_relatorio([run], "par", [2, 6], 4000)
    impresso = _inferior_impresso(texto)

    painel = bs.painel_diario(run.trades, Unidade("u", [run]).calendario)
    esperado = bs.ic_profit_factor(
        painel, bloco_medio=5.0, n_reamostras=4000, seed=20260907
    )
    iid = bs.ic_profit_factor(painel, bloco_medio=None, n_reamostras=4000, seed=20260907)

    assert impresso == pytest.approx(esperado.inferior, abs=5e-4)
    assert impresso != pytest.approx(esperado.superior, abs=5e-4)
    assert impresso != pytest.approx(iid.inferior, abs=5e-4)


def test_o_relatorio_declara_o_esquema_pre_registrado(tmp_path):
    texto = monta_relatorio([_run_bom(tmp_path)], "par", [2, 6], 500)
    assert "zeros incluidos" in texto
    assert "ADR-019" in texto
    # E mostra quantos pregoes de fato tiveram trade.
    assert "40 pregoes com trade, de 150 pregoes no periodo (27%)" in texto


def test_a_sensibilidade_ao_esquema_aparece_no_relatorio(tmp_path):
    texto = monta_relatorio([_run_bom(tmp_path)], "par", [2, 6], 500)
    assert "Sensibilidade ao esquema" in texto
    assert "NAO e o" in texto


def test_o_texto_do_DSR_nao_afirma_direcao_de_vies(tmp_path):
    texto = monta_relatorio([_run_bom(tmp_path)], "par", [2, 6], 500)
    assert "direcao do erro e desconhecida" in texto
    # A afirmacao antiga, refutada em 08/09, nao pode voltar.
    assert "O DSR real e pior que o impresso" not in texto
