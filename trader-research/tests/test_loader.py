from __future__ import annotations

import json

import pytest

from conftest import escreve_run, serie_utc, trade_json
from trader_research.loader import (
    CAPITAL_PADRAO,
    ReguaDivergente,
    exige_mesma_regua,
    exige_nao_experimental,
    load_run,
)
from trader_research.series import et_date


def _run(tmp_path, nome, **kw):
    dia = serie_utc(1)[0]
    trades = [trade_json(exit_iso=dia, net_pnl="10", result_in_r="0.1")]
    return load_run(escreve_run(tmp_path / nome, trades, **kw))


def test_juntar_run_com_e_sem_flatten_e_erro(tmp_path):
    a = _run(tmp_path, "a.json", session_flatten="15:45")
    b = _run(tmp_path, "b.json", session_flatten=None)
    with pytest.raises(ReguaDivergente):
        exige_mesma_regua([a, b])


def test_juntar_runs_com_slippage_diferente_e_erro(tmp_path):
    a = _run(tmp_path, "a.json", slippage_bps="2")
    b = _run(tmp_path, "b.json", slippage_bps="4")
    with pytest.raises(ReguaDivergente):
        exige_mesma_regua([a, b])


def test_mesma_regua_passa(tmp_path):
    a = _run(tmp_path, "a.json")
    b = _run(tmp_path, "b.json")
    # O `None` final e o dimensionamento: nenhum run de teste declara modo,
    # e a ADR-020 so passou a grava-lo em 08/09/2026.
    assert exige_mesma_regua([a, b]) == ("2", "15:45", "ibkr-fixed-us", "2", None)


def test_modelos_de_comissao_diferentes_nao_somam(tmp_path):
    # A comissao por acao da IBKR custa 9,9x o modelo fixo antigo na amostra
    # medida: somar os dois produz um numero que nao descreve mundo nenhum,
    # como somar com e sem flatten.
    a = _run(tmp_path, "a.json", commission_model="ibkr-fixed-us")
    b = _run(tmp_path, "b.json", commission_model="fixed-0.35")
    with pytest.raises(ReguaDivergente, match="comissao"):
        exige_mesma_regua([a, b])


def test_descontos_no_alvo_diferentes_nao_somam(tmp_path):
    a = _run(tmp_path, "a.json", limit_fill_haircut_bps="2")
    b = _run(tmp_path, "b.json", limit_fill_haircut_bps="0")
    with pytest.raises(ReguaDivergente, match="desconto no alvo"):
        exige_mesma_regua([a, b])


def test_run_sem_modelo_de_comissao_nao_soma_com_run_novo(tmp_path):
    # Run anterior ao §5.6 nao declara o modelo. Ele nao pode entrar num
    # agregado com runs que declaram: sao mundos diferentes.
    a = _run(tmp_path, "a.json", commission_model=None)
    b = _run(tmp_path, "b.json", commission_model="ibkr-fixed-us")
    with pytest.raises(ReguaDivergente):
        exige_mesma_regua([a, b])


def test_run_experimental_nao_entra_em_relatorio_de_gate(tmp_path):
    a = _run(tmp_path, "a.json", experimental=True, label="ablacao")
    with pytest.raises(ValueError, match="experimentais"):
        exige_nao_experimental([a])


def test_capital_padrao_quando_o_json_nao_traz(tmp_path):
    a = _run(tmp_path, "a.json", initial_capital=None)
    assert a.initial_capital == CAPITAL_PADRAO


def test_capital_do_json_tem_prioridade(tmp_path):
    a = _run(tmp_path, "a.json", initial_capital="50000")
    assert int(a.initial_capital) == 50_000


def test_export_de_backtest_e_recusado_com_mensagem_util(tmp_path):
    caminho = tmp_path / "b.json"
    caminho.write_text(json.dumps({"symbol": "TST"}), encoding="utf-8")
    with pytest.raises(ValueError, match="walkforward"):
        load_run(caminho)


def test_flatten_antigo_do_live_vira_end_of_day(tmp_path):
    dia = serie_utc(1)[0]
    trades = [
        trade_json(
            exit_iso=dia,
            net_pnl="10",
            result_in_r="0.1",
            exit_reason="manual",
            journal={"forced_exit": "session_flatten"},
        )
    ]
    run = load_run(escreve_run(tmp_path / "a.json", trades))
    assert run.trades[0].exit_reason == "manual"
    assert run.trades[0].exit_reason_efetivo == "end_of_day"


def test_manual_sem_forced_exit_continua_manual(tmp_path):
    dia = serie_utc(1)[0]
    trades = [
        trade_json(exit_iso=dia, net_pnl="10", result_in_r="0.1", exit_reason="manual")
    ]
    run = load_run(escreve_run(tmp_path / "a.json", trades))
    assert run.trades[0].exit_reason_efetivo == "manual"


def test_data_de_referencia_e_a_de_nova_york():
    from datetime import datetime, timezone

    # 20/12/2025 as 21h05 UTC = 16h05 ET do MESMO dia (horario padrao).
    momento = datetime(2025, 12, 19, 21, 5, tzinfo=timezone.utc)
    assert et_date(momento).isoformat() == "2025-12-19"
    # 20/12 as 00h30 UTC ainda e 19/12 as 19h30 em Nova York.
    virada = datetime(2025, 12, 20, 0, 30, tzinfo=timezone.utc)
    assert et_date(virada).isoformat() == "2025-12-19"


def test_horario_de_verao_nao_desloca_o_pregao():
    from datetime import datetime, timezone

    # Julho: 20h00 UTC = 16h00 EDT do mesmo dia.
    verao = datetime(2025, 7, 15, 20, 0, tzinfo=timezone.utc)
    assert et_date(verao).isoformat() == "2025-07-15"
