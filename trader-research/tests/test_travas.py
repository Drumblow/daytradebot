"""As travas que fazem o comando falhar em vez de produzir numero errado.

Cada uma corresponde a um modo de falha que a revisao adversarial de
08/09/2026 demonstrou: arquivo repetido inflando o pool, run velho sem
calendario rodando um esquema que nao e o pre-registrado, e ativo contado duas
vezes num "portfolio de 8 pares" que tem 7 ativos.
"""

from __future__ import annotations

import pytest

from conftest import calendario_util, escreve_run, serie_utc, trade_json
from trader_research.loader import (
    ReguaDivergente,
    SemCalendario,
    exige_calendario,
    exige_mesma_regua,
    load_run,
    load_runs,
    simbolos_repetidos,
)

MODO_A = {"risk_per_trade_pct": "1", "capital_fraction": "1", "max_notional_multiple": "1"}
MODO_B = {"risk_per_trade_pct": "0.15", "capital_fraction": "1", "max_notional_multiple": "1"}

CALENDARIO = calendario_util(120)


def _arquivo(tmp_path, nome, symbol="AAA", **kw):
    dias = serie_utc(3)
    trades = [
        trade_json(symbol=symbol, exit_iso=d, net_pnl="10", result_in_r="0.1")
        for d in dias
    ]
    return escreve_run(tmp_path / nome, trades, sessions=CALENDARIO, **kw)


def test_o_mesmo_arquivo_duas_vezes_e_recusado(tmp_path):
    a = _arquivo(tmp_path, "a.json")
    with pytest.raises(ValueError, match="duas vezes"):
        load_runs([a, a])


def test_o_mesmo_arquivo_por_caminhos_diferentes_e_recusado(tmp_path):
    a = _arquivo(tmp_path, "a.json")
    outro = tmp_path / "sub" / ".." / "a.json"
    outro.parent.mkdir(exist_ok=True)
    with pytest.raises(ValueError, match="duas vezes"):
        load_runs([a, outro])


def test_dois_runs_da_mesma_combinacao_sao_recusados(tmp_path):
    a = _arquivo(tmp_path, "a.json")
    b = _arquivo(tmp_path, "copia.json")
    with pytest.raises(ValueError, match="copia"):
        load_runs([a, b])


def test_simbolos_diferentes_passam(tmp_path):
    a = _arquivo(tmp_path, "a.json", symbol="AAA")
    b = _arquivo(tmp_path, "b.json", symbol="BBB")
    assert len(load_runs([a, b])) == 2


def test_run_sem_calendario_nao_roda_o_esquema_pre_registrado(tmp_path):
    dias = serie_utc(3)
    trades = [trade_json(exit_iso=d, net_pnl="10", result_in_r="0.1") for d in dias]
    velho = escreve_run(tmp_path / "velho.json", trades, sem_calendario=True)
    run = load_run(velho)
    assert run.oos_sessions is None
    with pytest.raises(SemCalendario, match="oos_sessions"):
        exige_calendario([run])


def test_run_com_calendario_passa(tmp_path):
    run = load_run(_arquivo(tmp_path, "a.json"))
    assert run.oos_sessions
    exige_calendario([run])


def test_ativo_em_duas_estrategias_e_reportado(tmp_path):
    import json

    a = _arquivo(tmp_path, "a.json", symbol="AVUV")
    b = _arquivo(tmp_path, "b.json", symbol="AVUV")
    # Muda a estrategia do segundo para nao cair na trava de duplicata.
    bruto = json.loads(b.read_text(encoding="utf-8"))
    bruto["strategy_id"] = "outra"
    b.write_text(json.dumps(bruto), encoding="utf-8")
    c = _arquivo(tmp_path, "c.json", symbol="SLYV")

    runs = load_runs([a, b, c])
    repetidos = simbolos_repetidos(runs)
    assert set(repetidos) == {"AVUV"}
    assert sorted(repetidos["AVUV"]) == ["outra", "tst"]
    # Tres pares, dois ativos: a diversificacao e menor que a contagem sugere.
    assert len({r.symbol for r in runs}) == 2


def test_sem_repeticao_o_mapa_sai_vazio(tmp_path):
    runs = load_runs(
        [_arquivo(tmp_path, "a.json", symbol="AAA"), _arquivo(tmp_path, "b.json", symbol="BBB")]
    )
    assert simbolos_repetidos(runs) == {}


def test_modos_de_sizing_diferentes_nao_sao_somaveis(tmp_path):
    """ADR-020: o eixo mais traicoeiro da regua.

    Dois modos de dimensionamento produzem PF em R e avg R praticamente
    iguais — as metricas que o gate le — com P&L em $ completamente
    diferentes: nos
    214 trades medidos, US$ 14.574 no modo A contra US$ 4.128 no modo B. Sem
    esta trava, somar os dois daria um numero que nao descreve nenhum dos
    dois, e nada no relatorio denunciaria.
    """
    a = _arquivo(tmp_path, "a.json", symbol="AAA", sizing=MODO_A)
    b = _arquivo(tmp_path, "b.json", symbol="BBB", sizing=MODO_B)
    with pytest.raises(ReguaDivergente, match="sizing"):
        exige_mesma_regua(load_runs([a, b]))


def test_mesmo_modo_de_sizing_soma(tmp_path):
    a = _arquivo(tmp_path, "a.json", symbol="AAA", sizing=MODO_A)
    b = _arquivo(tmp_path, "b.json", symbol="BBB", sizing=MODO_A)
    assert exige_mesma_regua(load_runs([a, b]))


def test_a_ordem_das_chaves_do_sizing_nao_conta(tmp_path):
    invertido = dict(reversed(list(MODO_A.items())))
    a = _arquivo(tmp_path, "a.json", symbol="AAA", sizing=MODO_A)
    b = _arquivo(tmp_path, "b.json", symbol="BBB", sizing=invertido)
    assert exige_mesma_regua(load_runs([a, b]))


def test_runs_antigos_sem_sizing_continuam_somaveis(tmp_path):
    """Todo run anterior a 08/09/2026 nao declara dimensionamento.

    A regua tem de continuar fechando entre eles — senao a trava nova
    invalidaria os 8 runs do §5.6 que ela nem deveria tocar.
    """
    a = _arquivo(tmp_path, "a.json", symbol="AAA")
    b = _arquivo(tmp_path, "b.json", symbol="BBB")
    assert exige_mesma_regua(load_runs([a, b]))


def test_run_com_e_sem_sizing_nao_sao_somaveis(tmp_path):
    a = _arquivo(tmp_path, "a.json", symbol="AAA", sizing=MODO_A)
    b = _arquivo(tmp_path, "b.json", symbol="BBB")
    with pytest.raises(ReguaDivergente, match="sizing"):
        exige_mesma_regua(load_runs([a, b]))
