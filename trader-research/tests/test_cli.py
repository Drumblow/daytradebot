from __future__ import annotations

from decimal import Decimal

import pytest

from conftest import calendario_util, escreve_run, serie_utc, trade_json
from trader_research.cli import Unidade, agrupa, main, monta_relatorio
from trader_research.loader import load_run


#: Os runs reais cobrem ~270 pregoes; um calendario curto faria a trava de
#: L/n do bootstrap recusar o bloco de 10 e mudar o que os testes medem.
CALENDARIO = calendario_util(120)


def _run(tmp_path, nome, symbol, dias_iso, pnls):
    trades = [
        trade_json(symbol=symbol, exit_iso=d, net_pnl=str(p), result_in_r=str(p / 100.0))
        for d, p in zip(dias_iso, pnls)
    ]
    caminho = escreve_run(tmp_path / nome, trades, sessions=CALENDARIO)
    return load_run(caminho)


def test_pool_ordena_por_tempo_e_nao_por_arquivo(tmp_path):
    # Dois simbolos que negociam nos MESMOS pregoes. Concatenando arquivo a
    # arquivo, a curva de equity percorreria 2025-2026 duas vezes e o
    # drawdown observado sairia de uma sequencia que nunca existiu.
    dias = serie_utc(4)
    a = _run(tmp_path, "a.json", "AAA", dias, [100.0, -400.0, 50.0, 50.0])
    b = _run(tmp_path, "b.json", "BBB", dias, [-50.0, 300.0, -20.0, -30.0])
    u = Unidade("pool", [a, b])
    tempos = [t.exit_time for t in u.trades]
    assert tempos == sorted(tempos)
    # A cada pregao os dois simbolos aparecem juntos, nao em blocos.
    simbolos = [t.symbol for t in u.trades]
    assert simbolos[:2] == ["AAA", "BBB"]
    assert simbolos != ["AAA"] * 4 + ["BBB"] * 4


def test_par_isolado_nao_muda_com_a_ordenacao(tmp_path):
    dias = serie_utc(3)
    a = _run(tmp_path, "a.json", "AAA", dias, [10.0, -5.0, 7.0])
    u = Unidade("par", [a])
    assert [t.exit_time for t in u.trades] == [t.exit_time for t in a.trades]
    assert [float(t.net_pnl) for t in u.trades] == [10.0, -5.0, 7.0]


def test_capital_do_pool_e_a_soma_dos_runs(tmp_path):
    dias = serie_utc(2)
    a = _run(tmp_path, "a.json", "AAA", dias, [1.0, 2.0])
    b = _run(tmp_path, "b.json", "BBB", dias, [3.0, 4.0])
    assert Unidade("pool", [a, b]).capital == Decimal(200_000)
    assert Unidade("pool", [a, b], Decimal(238_000)).capital == Decimal(238_000)


def test_pool_avisa_que_o_capital_nao_e_o_da_conta_real(tmp_path):
    dias = serie_utc(6)
    a = _run(tmp_path, "a.json", "AAA", dias, [100.0, -80.0, 40.0, -30.0, 20.0, 10.0])
    b = _run(tmp_path, "b.json", "BBB", dias, [-40.0, 90.0, -20.0, 30.0, -10.0, 25.0])
    texto = monta_relatorio([a, b], "estrategia", [2, 6], 200)
    assert "dividem UMA conta" in texto
    assert "200,000" in texto


def test_pool_com_capital_forcado_avisa_que_a_conta_nao_existe(tmp_path):
    # O caso mais perigoso: P&L gerado por 2 backtests de 100k cada, com o
    # percentual medido contra 238k. Nenhuma conta real produziria isso.
    dias = serie_utc(6)
    a = _run(tmp_path, "a.json", "AAA", dias, [100.0, -80.0, 40.0, -30.0, 20.0, 10.0])
    b = _run(tmp_path, "b.json", "BBB", dias, [-40.0, 90.0, -20.0, 30.0, -10.0, 25.0])
    texto = monta_relatorio([a, b], "estrategia", [2, 6], 200, Decimal(238_000))
    assert "nao descreve nenhuma conta que exista" in texto
    assert "200,000" in texto and "238,000" in texto


def test_par_isolado_nao_leva_o_aviso_de_pool(tmp_path):
    dias = serie_utc(6)
    a = _run(tmp_path, "a.json", "AAA", dias, [100.0, -80.0, 40.0, -30.0, 20.0, 10.0])
    texto = monta_relatorio([a], "par", [2, 6], 200)
    assert "dividem UMA conta" not in texto


def test_portfolio_sem_capital_e_recusado(tmp_path, capsys):
    dias = serie_utc(4)
    a = _run(tmp_path, "a.json", "AAA", dias, [10.0, -5.0, 7.0, 2.0])
    codigo = main([str(a.caminho), "--por", "portfolio", "--reamostras", "50"])
    assert codigo == 2
    assert "exige --capital" in capsys.readouterr().err


def test_portfolio_com_capital_roda(tmp_path):
    dias = serie_utc(6)
    a = _run(tmp_path, "a.json", "AAA", dias, [10.0, -5.0, 7.0, 2.0, -3.0, 4.0])
    saida = tmp_path / "r.md"
    codigo = main(
        [
            str(a.caminho),
            "--por",
            "portfolio",
            "--capital",
            "238000",
            "--reamostras",
            "50",
            "--out",
            str(saida),
        ]
    )
    assert codigo == 0
    assert "portfolio (1 pares)" in saida.read_text(encoding="utf-8")


def test_agrupamento_desconhecido_e_erro(tmp_path):
    dias = serie_utc(2)
    a = _run(tmp_path, "a.json", "AAA", dias, [1.0, 2.0])
    with pytest.raises(ValueError):
        agrupa([a], "por-hora")
