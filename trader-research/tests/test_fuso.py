"""A conversao de fuso, testada com casos que DISCRIMINAM.

A revisao adversarial de 08/09/2026 mostrou que os testes anteriores passavam
identicos com `et_date = lambda m: m.date()`, e que trocar `exit_time` por
`entry_time` no painel, no P&L por dia e no P&L por mes nao quebrava nada —
porque nos 214 trades reais nenhum tem data ET de entrada diferente da de
saida, e nenhum tem data UTC diferente da ET (sao intraday com flatten as
15h45). A trava existe para o dia em que isso deixar de valer: trade
overnight (`hold_overnight` do plano §5.7, v2 com short) ou saida em sessao
estendida. Estes testes fabricam esses dois casos.
"""

from __future__ import annotations

from datetime import datetime, timezone

import pytest

from conftest import calendario_util, escreve_run, trade_json
from trader_research import bootstrap as bs
from trader_research.loader import load_run
from trader_research.series import et_date, pnl_por_dia, pnl_por_mes

# 19/12/2025 as 23h30 UTC = 18h30 ET (EST): mesmo dia em Nova York, dia
# seguinte em UTC nao -- ainda 19/12. Ja 20/12 as 01h30 UTC e 19/12 as 20h30
# ET: dias civis DIFERENTES nos dois fusos.
SAIDA_APOS_O_PREGAO = "2025-12-20T01:30:00Z"  # 19/12 20h30 ET
ENTRADA_NA_VESPERA = "2025-12-18T19:00:00Z"  # 18/12 14h00 ET


def test_et_date_discrimina_de_verdade():
    # Em UTC seria 20/12; em Nova York e 19/12.
    momento = datetime(2025, 12, 20, 1, 30, tzinfo=timezone.utc)
    assert momento.date().isoformat() == "2025-12-20"
    assert et_date(momento).isoformat() == "2025-12-19"


def test_virada_no_horario_de_verao_tambem_discrimina():
    # 16/07/2025 as 00h30 UTC = 15/07 as 20h30 EDT.
    momento = datetime(2025, 7, 16, 0, 30, tzinfo=timezone.utc)
    assert momento.date().isoformat() == "2025-07-16"
    assert et_date(momento).isoformat() == "2025-07-15"


def _run_overnight(tmp_path):
    """Um trade que entra num dia ET e sai no seguinte, fechando apos o sino."""
    bruto = trade_json(
        exit_iso=SAIDA_APOS_O_PREGAO, net_pnl="100", result_in_r="1.0"
    )
    bruto["entry_time"] = ENTRADA_NA_VESPERA
    calendario = ["2025-12-18", "2025-12-19"] + calendario_util(30, "2025-12-22")
    return load_run(escreve_run(tmp_path / "r.json", [bruto], sessions=calendario))


def test_o_dia_do_painel_e_o_da_SAIDA_nao_o_da_entrada(tmp_path):
    run = _run_overnight(tmp_path)
    t = run.trades[0]
    assert et_date(t.entry_time).isoformat() == "2025-12-18"
    assert et_date(t.exit_time).isoformat() == "2025-12-19"

    painel = bs.painel_diario(run.trades, run.oos_sessions)
    ativo = [d for d, n in zip(painel.dias, painel.n_trades) if n > 0]
    assert [d.isoformat() for d in ativo] == ["2025-12-19"]


def test_pnl_por_dia_e_por_mes_usam_a_data_ET_da_saida(tmp_path):
    run = _run_overnight(tmp_path)
    dias = pnl_por_dia(run.trades)
    assert [d.isoformat() for d in dias] == ["2025-12-19"]
    assert list(pnl_por_mes(run.trades)) == [(2025, 12)]


def test_a_data_UTC_da_saida_seria_OUTRA(tmp_path):
    # O ponto do teste anterior: em UTC o mesmo trade cairia em 20/12, que e
    # sabado e nem sequer e pregao.
    run = _run_overnight(tmp_path)
    assert run.trades[0].exit_time.date().isoformat() == "2025-12-20"
    assert run.trades[0].exit_time.date() not in set(run.oos_sessions)


def test_saida_em_dia_que_nao_e_pregao_pelo_calendario_e_erro(tmp_path):
    # Se alguem "simplificar" a conversao para UTC, a data cai fora do
    # calendario e o painel recusa em vez de inventar um pregao.
    run = _run_overnight(tmp_path)
    with pytest.raises(ValueError, match="nao estao no calendario"):
        bs.painel_diario(run.trades, [d for d in run.oos_sessions if d.day != 19])
