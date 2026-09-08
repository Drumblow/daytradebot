from __future__ import annotations

import numpy as np
import pytest

from conftest import escreve_run, serie_utc, trade_json
from trader_research import bootstrap as bs
from trader_research.loader import load_run


def _trades(pnls_por_dia: list[list[float]]):
    """Um dia por lista interna; cada float e um trade daquele dia."""
    dias = serie_utc(len(pnls_por_dia))
    brutos = []
    for dia, pnls in zip(dias, pnls_por_dia):
        for p in pnls:
            brutos.append(
                trade_json(
                    exit_iso=dia,
                    net_pnl=str(p),
                    result_in_r=str(p / 100.0),
                    risk_amount="100",
                )
            )
    return brutos


def test_ponto_do_bootstrap_e_o_pf_por_TRADE_nao_o_agregado_do_dia(tmp_path):
    # Um unico dia com +100 e -80. PF por trade = 1,25; se alguem agregasse o
    # dia antes (net +20, sem perda) o PF seria infinito. A diferenca entre os
    # dois numeros e a razao de este pacote reamostrar o dia mas somar trades.
    caminho = escreve_run(tmp_path / "r.json", _trades([[100.0, -80.0]]))
    run = load_run(caminho)
    painel = bs.painel_diario(run.trades)
    assert painel.lucro_bruto[0] == pytest.approx(100.0)
    assert painel.perda_bruta[0] == pytest.approx(80.0)
    ic = bs.ic_profit_factor(
        painel, bloco_medio=5.0, n_reamostras=50, permitir_bloco_longo=True
    )
    assert ic.ponto == pytest.approx(1.25)


def test_indices_estacionarios_sao_deterministicos_com_a_mesma_seed():
    a = bs.indices_estacionarios(30, 5.0, 200, np.random.default_rng(7), permitir_bloco_longo=True)
    b = bs.indices_estacionarios(30, 5.0, 200, np.random.default_rng(7), permitir_bloco_longo=True)
    assert np.array_equal(a, b)
    c = bs.indices_estacionarios(30, 5.0, 200, np.random.default_rng(8), permitir_bloco_longo=True)
    assert not np.array_equal(a, c)


def test_bloco_de_tamanho_1_equivale_a_iid_em_distribuicao():
    # L=1 => p=1 => todo passo comeca bloco novo => reamostragem iid.
    idx = bs.indices_estacionarios(40, 1.0, 500, np.random.default_rng(1))
    seguintes = (idx[:, :-1] + 1) % 40
    iguais = float(np.mean(idx[:, 1:] == seguintes))
    # Com 40 posicoes, coincidencia por acaso fica em ~1/40.
    assert iguais < 0.10


def test_bloco_grande_preserva_a_sequencia():
    idx = bs.indices_estacionarios(
        40, 20.0, 500, np.random.default_rng(1), permitir_bloco_longo=True
    )
    seguintes = (idx[:, :-1] + 1) % 40
    continua = float(np.mean(idx[:, 1:] == seguintes))
    assert continua > 0.80


def test_indices_ficam_no_intervalo():
    for L in (1.0, 5.0, 10.0):
        idx = bs.indices_estacionarios(
            17, L, 300, np.random.default_rng(3), permitir_bloco_longo=True
        )
        assert idx.min() >= 0 and idx.max() <= 16
        assert idx.shape == (300, 17)


def test_bloco_menor_que_um_pregao_e_erro():
    with pytest.raises(ValueError):
        bs.indices_estacionarios(10, 0.5, 5, np.random.default_rng(0))


def test_regime_persistente_alarga_muito_o_IC_em_blocos(tmp_path):
    # 30 pregoes de ganho seguidos de 30 de perda -- uma unica troca de regime,
    # o caso que o plano descreve ("o veredito muda com o esquema"). Sob iid
    # toda reamostra mistura os dois regimes meio a meio e o IC encolhe; em
    # blocos, uma reamostra pode cair quase toda dentro de um regime.
    padrao = [[120.0]] * 30 + [[-100.0]] * 30
    caminho = escreve_run(tmp_path / "r.json", _trades(padrao))
    painel = bs.painel_diario(load_run(caminho).trades)
    iid = bs.ic_profit_factor(painel, bloco_medio=None, n_reamostras=4000, seed=11)
    blocos = bs.ic_profit_factor(
        painel, bloco_medio=10.0, n_reamostras=4000, seed=11, permitir_bloco_longo=True
    )
    assert (blocos.superior - blocos.inferior) > 3 * (iid.superior - iid.inferior)
    assert blocos.inferior < iid.inferior


def test_bloco_NEM_SEMPRE_alarga_o_IC(tmp_path):
    # Contraexemplo deliberado, e por isso um teste: quando o comprimento do
    # bloco e multiplo do periodo da alternancia, cada bloco ja carrega o ciclo
    # inteiro, a reamostra fica mais parecida com a serie original e o IC
    # ENCOLHE em relacao ao iid. "Bloco = mais conservador" e falso como regra
    # geral; o que blocos fazem e preservar a estrutura, para o bem e para o
    # mal. Por isso o relatorio imprime os dois esquemas lado a lado.
    padrao = []
    for i in range(10):
        padrao += [[120.0 if i % 2 == 0 else -100.0]] * 6
    caminho = escreve_run(tmp_path / "r.json", _trades(padrao))
    painel = bs.painel_diario(load_run(caminho).trades)
    iid = bs.ic_profit_factor(painel, bloco_medio=None, n_reamostras=4000, seed=11)
    blocos = bs.ic_profit_factor(
        painel, bloco_medio=12.0, n_reamostras=4000, seed=11, permitir_bloco_longo=True
    )
    assert (blocos.superior - blocos.inferior) < (iid.superior - iid.inferior)


def test_sem_pregao_perdedor_o_pf_e_infinito(tmp_path):
    caminho = escreve_run(tmp_path / "r.json", _trades([[50.0]] * 8))
    painel = bs.painel_diario(load_run(caminho).trades)
    ic = bs.ic_profit_factor(
        painel, bloco_medio=5.0, n_reamostras=500, seed=2, permitir_bloco_longo=True
    )
    assert ic.ponto == float("inf")
    assert ic.fracao_infinita == pytest.approx(1.0)
    assert ic.superior == float("inf")


def test_ic_avg_r_bate_com_a_media_simples(tmp_path):
    caminho = escreve_run(tmp_path / "r.json", _trades([[100.0, -50.0], [25.0]]))
    painel = bs.painel_diario(load_run(caminho).trades)
    ic = bs.ic_avg_r(
        painel, bloco_medio=5.0, n_reamostras=100, permitir_bloco_longo=True
    )
    assert ic.ponto == pytest.approx((1.0 - 0.5 + 0.25) / 3)


def test_ic_net_bate_com_a_soma(tmp_path):
    caminho = escreve_run(tmp_path / "r.json", _trades([[100.0, -50.0], [25.0]]))
    painel = bs.painel_diario(load_run(caminho).trades)
    ic = bs.ic_net(
        painel, bloco_medio=5.0, n_reamostras=100, permitir_bloco_longo=True
    )
    assert ic.ponto == pytest.approx(75.0)


def test_trade_zerado_conta_como_perda_igual_ao_motor(tmp_path):
    # `metrics.rs` usa `if pnl > 0 { ganho } else { perda }`: zero e perda.
    caminho = escreve_run(tmp_path / "r.json", _trades([[0.0, 10.0]]))
    painel = bs.painel_diario(load_run(caminho).trades)
    assert painel.perda_bruta[0] == pytest.approx(0.0)
    assert painel.lucro_bruto[0] == pytest.approx(10.0)
    assert painel.n_trades[0] == 2
