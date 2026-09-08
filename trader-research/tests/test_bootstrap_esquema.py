"""O esquema pre-registrado e as duas propriedades que definem Politis-Romano.

Arquivo separado de `test_bootstrap.py` porque cobre outra coisa: la estao as
propriedades basicas do reamostrador; aqui esta o que a revisao adversarial de
08/09/2026 mostrou que faltava — que o painel siga o esquema PRE-REGISTRADO no
ADR-019 secao 8 ("P&L diario de todos os pregoes, zeros incluidos"), que o
bloco seja geometrico e nao fixo, que o reinicio seja uniforme, e que os
limites do IC (nao so o ponto) sejam conferidos contra caso conhecido.
"""

from __future__ import annotations

import math

import numpy as np
import pytest

from conftest import calendario_util, escreve_run, serie_utc, trade_json
from trader_research import bootstrap as bs
from trader_research.loader import load_run


# ---------------------------------------------------------------------------
# Esquema pre-registrado: TODOS os pregoes, zeros incluidos.
# ---------------------------------------------------------------------------


def test_calendario_inclui_os_pregoes_sem_trade(tmp_path):
    calendario = calendario_util(40)
    dias = serie_utc(40)
    brutos = [
        trade_json(exit_iso=dias[i], net_pnl=str(v), result_in_r=str(v / 100.0))
        for i, v in ((0, 100.0), (10, -50.0), (30, 25.0))
    ]
    run = load_run(escreve_run(tmp_path / "r.json", brutos, sessions=calendario))
    painel = bs.painel_diario(run.trades, run.oos_sessions)
    assert len(painel) == 40
    assert painel.pregoes_ativos == 3
    assert painel.esquema == "calendario"
    assert painel.net.sum() == pytest.approx(75.0)
    assert (painel.n_trades == 0).sum() == 37


def test_sem_calendario_o_painel_encolhe_para_os_dias_ativos(tmp_path):
    calendario = calendario_util(40)
    dias = serie_utc(40)
    brutos = [
        trade_json(exit_iso=dias[i], net_pnl=str(v), result_in_r=str(v / 100.0))
        for i, v in ((0, 100.0), (10, -50.0), (30, 25.0))
    ]
    run = load_run(escreve_run(tmp_path / "r.json", brutos, sessions=calendario))
    painel = bs.painel_diario(run.trades)
    assert len(painel) == 3 and painel.esquema == "ativos"
    # O PONTO e o mesmo nos dois esquemas; o que muda e a reamostragem.
    completo = bs.painel_diario(run.trades, run.oos_sessions)
    assert completo.lucro_bruto.sum() == pytest.approx(painel.lucro_bruto.sum())
    assert completo.perda_bruta.sum() == pytest.approx(painel.perda_bruta.sum())


def test_o_esquema_muda_o_IC_de_verdade(tmp_path):
    # E o motivo de o esquema ser pre-registrado: com os MESMOS trades, o
    # calendario completo produz IC mais largo, porque a propria frequencia de
    # trades passa a ser aleatoria na reamostra.
    calendario = calendario_util(200)
    dias = serie_utc(200)
    brutos = []
    for i in range(0, 200, 10):
        v = 120.0 if (i // 10) % 3 else -100.0
        brutos.append(
            trade_json(exit_iso=dias[i], net_pnl=str(v), result_in_r=str(v / 100.0))
        )
    run = load_run(escreve_run(tmp_path / "r.json", brutos, sessions=calendario))
    completo = bs.ic_profit_factor(
        bs.painel_diario(run.trades, run.oos_sessions),
        bloco_medio=5.0,
        n_reamostras=4000,
        seed=3,
    )
    ativos = bs.ic_profit_factor(
        bs.painel_diario(run.trades),
        bloco_medio=5.0,
        n_reamostras=4000,
        seed=3,
        permitir_bloco_longo=True,
    )
    assert completo.ponto == pytest.approx(ativos.ponto)
    assert (completo.superior - completo.inferior) > (ativos.superior - ativos.inferior)


def test_trade_fora_do_calendario_e_erro(tmp_path):
    dias = serie_utc(3)
    brutos = [trade_json(exit_iso=d, net_pnl="10", result_in_r="0.1") for d in dias]
    run = load_run(escreve_run(tmp_path / "r.json", brutos))
    with pytest.raises(ValueError, match="nao estao no calendario"):
        bs.painel_diario(run.trades, run.oos_sessions[:1])


def test_bloco_longo_demais_para_a_serie_e_recusado():
    # 20 pregoes com bloco medio 5: L/n = 0,25. A reamostra vira rotacao da
    # serie em ~36% dos casos e o "IC95" perde cobertura.
    with pytest.raises(bs.BlocoLongoDemais, match="rotacao"):
        bs.indices_estacionarios(20, 5.0, 100, np.random.default_rng(0))
    # 270 pregoes (a ordem de grandeza real dos runs): legitimo.
    idx = bs.indices_estacionarios(270, 5.0, 10, np.random.default_rng(0))
    assert idx.shape == (10, 270)


# ---------------------------------------------------------------------------
# As duas propriedades que DEFINEM Politis-Romano.
# ---------------------------------------------------------------------------


def _corridas(idx: np.ndarray, n: int) -> list[int]:
    """Comprimento de cada bloco contiguo em cada reamostra."""
    saida = []
    for linha in idx:
        atual = 1
        for a, b in zip(linha[:-1], linha[1:]):
            if b == (a + 1) % n:
                atual += 1
            else:
                saida.append(atual)
                atual = 1
        saida.append(atual)
    return saida


def test_o_comprimento_do_bloco_e_GEOMETRICO_nao_fixo():
    # P(comprimento = k) = (1-p)^(k-1) * p. Um moving-block bootstrap (bloco
    # FIXO) passa nos testes de fracao media e falha aqui — e o docstring do
    # modulo afirma que o tamanho aleatorio e o que torna a serie
    # reamostrada estacionaria.
    n, L, B = 400, 5.0, 400
    idx = bs.indices_estacionarios(n, L, B, np.random.default_rng(11))
    corridas = np.array(_corridas(idx, n))
    p = 1.0 / L
    for k in (1, 2, 3, 4, 5):
        esperado = (1 - p) ** (k - 1) * p
        observado = float((corridas == k).mean())
        assert observado == pytest.approx(esperado, abs=0.02), f"k={k}"


def test_o_reinicio_do_bloco_e_UNIFORME_sobre_os_pregoes():
    # Um off-by-one no sorteio (integers(0, n-1), ou 1..n) deslocaria a massa
    # e deixaria um dos pregoes de fora.
    n, B = 50, 2000
    idx = bs.indices_estacionarios(n, 5.0, B, np.random.default_rng(12))
    contagem = np.bincount(idx.ravel(), minlength=n)
    assert contagem.size == n and contagem.min() > 0
    esperado = idx.size / n
    assert abs(contagem - esperado).max() < 5 * np.sqrt(esperado)


# ---------------------------------------------------------------------------
# Os LIMITES do IC, nao so o ponto.
# ---------------------------------------------------------------------------


def _painel(lucros, perdas):
    n = len(lucros)
    return bs.PainelDiario(
        dias=list(range(n)),
        lucro_bruto=np.array(lucros, dtype=float),
        perda_bruta=np.array(perdas, dtype=float),
        lucro_bruto_r=np.array(lucros, dtype=float) / 100.0,
        perda_bruta_r=np.array(perdas, dtype=float) / 100.0,
        soma_r=(np.array(lucros, dtype=float) - np.array(perdas, dtype=float)) / 100.0,
        n_trades=np.ones(n),
        n_ganhos=(np.array(lucros, dtype=float) > 0).astype(float),
        net=np.array(lucros, dtype=float) - np.array(perdas, dtype=float),
        esquema="calendario",
    )


def test_o_IC_contem_o_ponto_e_exclui_a_razao_invertida():
    # 20 pregoes alternando +100 e -50: PF = 1000/500 = 2,0 exato. Com 20
    # sorteios, P(so ganhadores) = 2^-20, entao os dois limites sao finitos e
    # o teste mede o que quer medir.
    lucros = [100.0 if i % 2 == 0 else 0.0 for i in range(20)]
    perdas = [0.0 if i % 2 == 0 else 50.0 for i in range(20)]
    ic = bs.ic_profit_factor(
        _painel(lucros, perdas), bloco_medio=None, n_reamostras=10_000, seed=7
    )
    assert ic.ponto == pytest.approx(2.0)
    assert ic.fracao_infinita == 0.0
    assert math.isfinite(ic.inferior) and math.isfinite(ic.superior)
    assert ic.inferior < ic.ponto < ic.superior
    # Inverter numerador e denominador em `_razao` jogaria a distribuicao
    # inteira em volta de 0,5 — fora deste intervalo.
    assert not (ic.inferior <= 0.5 <= ic.superior)


def test_percentil_corrige_a_massa_infinita():
    # 4 pregoes, so um perdedor: parte das reamostras nao pega nenhuma perda e
    # o PF vira infinito. P(nenhum perdedor em 4 sorteios) = (3/4)^4 = 31,6%.
    painel = _painel([100.0, 100.0, 100.0, 0.0], [0.0, 0.0, 0.0, 10.0])
    ic = bs.ic_profit_factor(painel, bloco_medio=None, n_reamostras=10_000, seed=5)
    assert ic.fracao_infinita == pytest.approx(0.316, abs=0.02)
    # Acima de 2,5% de massa infinita o limite superior E infinito; devolver
    # um numero finito seria inventa-lo.
    assert ic.superior == float("inf")
    assert math.isfinite(ic.inferior)


def test_amostra_pequena_com_poucos_perdedores_ja_degenera_o_IC():
    # 4 pregoes, 2 ganhadores e 2 perdedores: P(so ganhadores) = P(so
    # perdedores) = (1/2)^4 = 6,25% cada. O IC entao vai de 0 a infinito — e é
    # isso que o codigo deve devolver, nao um intervalo apresentavel. É o
    # motivo pratico de o esquema pre-registrado usar o calendario inteiro.
    ic = bs.ic_profit_factor(
        _painel([100.0, 0.0, 30.0, 0.0], [0.0, 50.0, 0.0, 20.0]),
        bloco_medio=None,
        n_reamostras=10_000,
        seed=5,
    )
    assert ic.fracao_infinita == pytest.approx(0.0625, abs=0.01)
    assert ic.inferior == 0.0
    assert ic.superior == float("inf")


def test_n_ganhos_e_o_unico_lugar_onde_zero_como_perda_e_observavel(tmp_path):
    # Somar 0.0 no balde de ganho ou no de perda nao muda soma nenhuma: a
    # regra `pnl > 0` do metrics.rs so aparece na CONTAGEM.
    dias = serie_utc(1)
    brutos = [
        trade_json(exit_iso=dias[0], net_pnl="0", result_in_r="0"),
        trade_json(exit_iso=dias[0], net_pnl="10", result_in_r="0.1"),
    ]
    run = load_run(escreve_run(tmp_path / "r.json", brutos))
    painel = bs.painel_diario(run.trades, run.oos_sessions)
    assert painel.n_trades[0] == 2
    assert painel.n_ganhos[0] == 1
