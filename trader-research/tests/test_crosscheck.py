"""O teste que importa: o que este pacote calcula bate com o motor?

Ele roda sobre os JSONs reais de `out/adr018/`. Se o Rust mudar uma convencao
(dia de referencia, denominador de share, criterio de ganho) e o Python nao,
este teste quebra antes de qualquer relatorio sair errado.
"""

from __future__ import annotations

import json

import pytest

from trader_research.crosscheck import confere
from trader_research.loader import load_run


def test_python_reproduz_o_motor_em_todo_run(caminhos_runs):
    falhas = []
    for caminho in caminhos_runs:
        run = load_run(caminho)
        divs = confere(run.trades, run.metrics, run.initial_capital)
        if divs:
            falhas.append((caminho.name, [str(d) for d in divs]))
    assert not falhas, f"divergencias contra o metrics.rs: {falhas}"


def test_todo_run_do_gate_tem_regua_declarada(caminhos_runs):
    for caminho in caminhos_runs:
        run = load_run(caminho)
        assert run.slippage_bps, f"{caminho.name}: sem slippage_bps"
        assert run.session_flatten, (
            f"{caminho.name}: sem session_flatten -- run anterior ao ADR-018, "
            "nao comparavel com o live"
        )
        assert not run.experimental, f"{caminho.name}: run experimental"


@pytest.mark.parametrize("campo", ["profit_factor", "profit_factor_r"])
def test_pf_do_motor_e_positivo_ou_nulo(caminhos_runs, campo):
    for caminho in caminhos_runs:
        run = load_run(caminho)
        valor = run.metrics.get(campo)
        assert valor is None or float(valor) >= 0


def test_campo_ausente_e_divergencia_nao_silencio(tmp_path):
    """Export truncado tem de FALHAR, nao passar batido.

    Antes desta trava, `metrics.get(campo)` devolvia None para campo ausente
    e `cmp_dec` tratava "None dos dois lados" como acordo: um JSON sem
    `profit_factor` seria aprovado sem ter sido conferido.
    """
    from conftest import escreve_run, serie_utc, trade_json
    from trader_research.crosscheck import AUSENTE

    dia = serie_utc(1)[0]
    caminho = escreve_run(
        tmp_path / "a.json",
        [trade_json(exit_iso=dia, net_pnl="10", result_in_r="0.1")],
    )
    bruto = json.loads(caminho.read_text(encoding="utf-8"))
    del bruto["selection"]["oos_metrics"]["profit_factor"]
    del bruto["selection"]["oos_metrics"]["trading_days"]
    caminho.write_text(json.dumps(bruto), encoding="utf-8")

    run = load_run(caminho)
    divs = confere(run.trades, run.metrics, run.initial_capital)
    campos = {d.campo for d in divs}
    assert campos == {"profit_factor", "trading_days"}
    assert all(d.rust == AUSENTE for d in divs)


def test_pf_nulo_legitimo_nao_e_divergencia(tmp_path):
    """Run sem nenhuma perda grava `profit_factor: null` -- isso e acordo."""
    from conftest import escreve_run, serie_utc, trade_json

    dias = serie_utc(2)
    caminho = escreve_run(
        tmp_path / "a.json",
        [
            trade_json(exit_iso=d, net_pnl="10", result_in_r="0.1")
            for d in dias
        ],
    )
    run = load_run(caminho)
    assert run.metrics["profit_factor"] is None
    assert confere(run.trades, run.metrics, run.initial_capital) == []
