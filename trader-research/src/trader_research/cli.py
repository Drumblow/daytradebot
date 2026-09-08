"""Relatorio estatistico do gate A (secao 5.3 do plano de lucratividade).

Uso:
    uv run trader-research out/adr018/wf19_*.json --out docs/reports/x.md

Nada aqui decide. O criterio "limite inferior do IC95 em blocos >= 1,0" e
PROPOSTA do ADR-019 secao 7 e continua proposta ate o dono adota-la; o
relatorio imprime o numero e diz que nao e gate.
"""

from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from decimal import Decimal
from pathlib import Path
from typing import Sequence

from . import bootstrap as bs
from . import crosscheck, drawdown, psr
from .concentration import Concentracao, concentracao
from .loader import (
    Run,
    exige_calendario,
    exige_mesma_regua,
    exige_nao_experimental,
    load_runs,
    simbolos_repetidos,
)
from .series import vetor_diario, vetor_r, vetor_risco

SEED = 20260907
BLOCOS = (5.0, 10.0)


@dataclass
class Unidade:
    """Um recorte analisado: um par (estrategia x ativo) ou uma estrategia."""

    nome: str
    runs: list[Run]

    capital_forcado: Decimal | None = None

    @property
    def trades(self):
        # ORDEM CRONOLOGICA, nao ordem dos arquivos. Concatenar run a run
        # produziria uma curva de equity que percorre 2025-2026 uma vez por
        # simbolo, e o drawdown observado do pool sairia de uma sequencia que
        # nunca existiu.
        todos = [t for r in self.runs for t in r.trades]
        return sorted(todos, key=lambda t: (t.exit_time, t.entry_time, t.symbol))

    @property
    def capital(self) -> Decimal:
        if self.capital_forcado is not None:
            return self.capital_forcado
        # Um pool de N pares roda com N contas de `initial_capital` (cada
        # instancia do backtest tem a sua). O drawdown do pool sai sobre a soma
        # delas, e por isso NAO e comparavel com a conta real: em producao as
        # 8 instancias dividem UMA conta (~238k, plano 2.5), onde o mesmo P&L
        # daria drawdown maior. O replay de conta compartilhada e o 6.4.
        return sum((r.initial_capital for r in self.runs), Decimal(0))

    @property
    def e_pool(self) -> bool:
        return len(self.runs) > 1

    @property
    def calendario(self) -> list:
        """Uniao dos pregoes cobertos pelos runs da unidade.

        Uniao, e nao intersecao: um pregao em que so um dos pares operou e um
        pregao do pool, com P&L igual ao daquele par. Excluir o dia porque os
        outros nao operaram jogaria fora informacao real.
        """
        dias: set = set()
        for r in self.runs:
            dias.update(r.oos_sessions or [])
        return sorted(dias)


def agrupa(
    runs: Sequence[Run], por: str, capital: Decimal | None = None
) -> list[Unidade]:
    if por == "par":
        return [Unidade(r.par, [r], capital) for r in runs]
    if por == "estrategia":
        chaves: dict[str, list[Run]] = {}
        for r in runs:
            chaves.setdefault(r.strategy_id, []).append(r)
        return [
            Unidade(
                f"{k} (pool de {len(v)})", sorted(v, key=lambda r: r.symbol), capital
            )
            for k, v in sorted(chaves.items())
        ]
    if por == "portfolio":
        return [Unidade(f"portfolio ({len(runs)} pares)", list(runs), capital)]
    raise ValueError(f"agrupamento desconhecido: {por}")


def _fmt(x: float, casas: int = 3) -> str:
    if x != x:
        return "n/d"
    if x == float("inf"):
        return "inf"
    if x == float("-inf"):
        return "-inf"
    return f"{x:.{casas}f}"


def bloco_unidade(u: Unidade, trials: list[int], n_boot: int) -> list[str]:
    trades = u.trades
    # ADR-019 secao 8: "P&L diario de TODOS os pregoes, zeros incluidos".
    painel = bs.painel_diario(trades, u.calendario)
    linhas: list[str] = [f"### {u.nome}", ""]

    gp, gl = painel.lucro_bruto.sum(), painel.perda_bruta.sum()
    gpr, glr = painel.lucro_bruto_r.sum(), painel.perda_bruta_r.sum()
    pf_ponto = float(gp / gl) if gl > 0 else float("inf")
    pfr_ponto = float(gpr / glr) if glr > 0 else float("inf")
    avg_r = float(painel.soma_r.sum() / painel.n_trades.sum())
    ativos = painel.pregoes_ativos
    linhas += [
        f"{len(trades)} trades OOS em {ativos} pregoes com trade, de "
        f"{len(painel)} pregoes no periodo ({ativos / len(painel):.0%}) | "
        f"PF em $ {_fmt(pf_ponto, 2)} | PF em R {_fmt(pfr_ponto, 2)} | "
        f"avg R {_fmt(avg_r)} | net US$ {painel.net.sum():,.0f}",
        "",
    ]

    # --- IC95 por bootstrap -------------------------------------------------
    linhas += [
        "| estatistica | esquema | ponto | IC95 inferior | IC95 superior |",
        "|---|---|---|---|---|",
    ]
    ic_iid = bs.ic_profit_factor(
        painel, bloco_medio=None, n_reamostras=n_boot, seed=SEED
    )
    # O L=5 e o bloco do CRITERIO: se ele nao for legitimo para este n, o
    # veredito nao existe e isso tem de estourar, nao virar linha em branco.
    ic_l5 = bs.ic_profit_factor(painel, bloco_medio=5.0, n_reamostras=n_boot, seed=SEED)
    # O L=10 e sensibilidade: pode nao caber num recorte curto sem invalidar
    # o resto do bloco.
    try:
        ic_l10 = bs.ic_profit_factor(
            painel, bloco_medio=10.0, n_reamostras=n_boot, seed=SEED
        )
    except bs.BlocoLongoDemais:
        ic_l10 = None
    ics = [
        ic_iid,
        ic_l5,
        bs.ic_profit_factor(
            painel, em_r=True, bloco_medio=5.0, n_reamostras=n_boot, seed=SEED
        ),
        bs.ic_avg_r(painel, bloco_medio=5.0, n_reamostras=n_boot, seed=SEED),
        bs.ic_net(painel, bloco_medio=5.0, n_reamostras=n_boot, seed=SEED),
    ]
    if ic_l10 is not None:
        ics.insert(2, ic_l10)
    else:
        linhas.append(
            f"| PF em $ | blocos L=10 | {_fmt(pf_ponto, 3)} | nao calculavel "
            f"(L/n = {10 / len(painel):.2f}) | nao calculavel |"
        )
    for ic in ics:
        esquema = "iid" if ic.bloco_medio is None else f"blocos L={ic.bloco_medio:g}"
        casas = 0 if ic.estatistica.startswith("net") else 3
        linhas.append(
            f"| {ic.estatistica} | {esquema} | {_fmt(ic.ponto, casas)} | "
            f"{_fmt(ic.inferior, casas)} | {_fmt(ic.superior, casas)} |"
        )
    linhas.append("")

    veredito = "PASSA" if ic_l5.inferior >= 1.0 else "NAO passa"
    linhas += [
        "**Criterio PROPOSTO (ADR-019 secao 7, nao e o gate vigente):** limite "
        f"inferior do IC95 do PF em blocos (L=5) = {_fmt(ic_l5.inferior)} "
        f"contra o piso de 1,0 -- **{veredito}**.",
        "",
    ]
    if ic_l5.fracao_infinita > 0:
        linhas += [
            f"> {ic_l5.fracao_infinita:.2%} das reamostras nao tiveram nenhum "
            "pregao perdedor (PF infinito). Os percentis sao corrigidos para "
            "isso; quando o quantil pedido cai dentro da massa infinita, o "
            "limite sai como `inf`.",
            "",
        ]

    # Sensibilidade ao esquema: o que sairia reamostrando SO os pregoes com
    # trade. Nao e o pre-registrado; esta aqui porque a diferenca e grande e
    # esconde-la seria escolher o numero depois de ver os dois.
    painel_ativos = bs.painel_diario(trades)
    try:
        ic_ativos = bs.ic_profit_factor(
            painel_ativos, bloco_medio=5.0, n_reamostras=n_boot, seed=SEED
        )
        alternativa = (
            f"{_fmt(ic_ativos.inferior)} "
            f"({'PASSA' if ic_ativos.inferior >= 1.0 else 'NAO passa'})"
        )
    except bs.BlocoLongoDemais:
        alternativa = (
            "nao calculavel -- com so "
            f"{len(painel_ativos)} pregoes ativos, um bloco de 5 e "
            f"{5 / max(len(painel_ativos), 1):.0%} da serie e o bootstrap "
            "circular degenera em rotacao"
        )
    linhas += [
        f"> Sensibilidade ao esquema: reamostrando **so os "
        f"{painel.pregoes_ativos} pregoes com trade** (esquema que NAO e o "
        f"pre-registrado), o mesmo limite daria {alternativa}.",
        "",
    ]

    # --- PSR / DSR ----------------------------------------------------------
    diario = vetor_diario(trades)
    por_trade = vetor_r(trades)
    linhas += [
        "| serie | n | Sharpe (frequencia da serie) | skew | curtose | PSR(0) "
        "| DSR faixa |",
        "|---|---|---|---|---|---|---|",
    ]
    for rotulo, serie in (("P&L diario", diario), ("R por trade", por_trade)):
        if serie.size < 3:
            linhas.append(
                f"| {rotulo} | {serie.size} | amostra insuficiente | | | | |"
            )
            continue
        res = psr.analisa(serie, trials)
        lo, hi = res.faixa_dsr
        linhas.append(
            f"| {rotulo} | {res.n} | {_fmt(res.sharpe)} | {_fmt(res.skew, 2)} | "
            f"{_fmt(res.kurtose, 2)} | {_fmt(res.psr_zero, 3)} | "
            f"[{_fmt(hi, 3)} ... {_fmt(lo, 3)}] |"
        )
    linhas += [
        "",
        f"A faixa do DSR vai de N={min(trials)} a N={max(trials)} tentativas, "
        "do mais generoso ao mais severo. **A direcao do erro e desconhecida**: "
        "sem `n_trials` registrado (item 3 das pendencias do ADR-019), a "
        "variancia entre tentativas e substituida pela variancia do estimador "
        "de UMA serie. Se as tentativas forem correlacionadas -- variacoes de "
        "parametro da mesma regra, que e o caso registrado --, o numero "
        "impresso e PESSIMISTA; so se cobrissem estrategias genuinamente "
        "diferentes ele seria otimista.",
        "",
    ]

    # --- Monte Carlo de drawdown -------------------------------------------
    capital = float(u.capital)
    linhas += [
        "| sizing | DD observado | DD mediano | DD p95 | DD p99 | P(DD > 10%) |",
        "|---|---|---|---|---|---|",
    ]
    for modo in drawdown.MODOS_PADRAO:
        mc = drawdown.monte_carlo(
            vetor_r(trades),
            vetor_risco(trades),
            capital=capital,
            modo=modo,
            n_simulacoes=n_boot,
            seed=SEED,
        )
        linhas.append(
            f"| {mc.modo} | {mc.dd_observado:.2f}% | {mc.dd_mediano:.2f}% | "
            f"{mc.dd_p95:.2f}% | {mc.dd_p99:.2f}% | {mc.prob_acima_de_10pct:.1%} |"
        )
    linhas += [
        "",
        "Os modos de fracao fixa **ignoram o teto de notional**, que hoje e o "
        "que realmente prende (plano 5.5). Sem o cap eles superestimam o risco "
        "tomado: sao teto, nao previsao.",
        "",
    ]
    if u.e_pool:
        soma = float(sum((r.initial_capital for r in u.runs), Decimal(0)))
        if u.capital_forcado is None:
            linhas += [
                f"> **Drawdown do pool sobre US$ {capital:,.0f}** -- a SOMA dos "
                f"{len(u.runs)} capitais iniciais do backtest. Em producao as "
                "instancias dividem UMA conta (~238k, plano 2.5), onde o mesmo "
                "P&L daria drawdown maior.",
                "",
            ]
        else:
            linhas += [
                f"> **Este drawdown nao descreve nenhuma conta que exista.** O "
                f"P&L foi gerado por {len(u.runs)} backtests independentes, cada "
                f"um dimensionando sobre o proprio capital (soma US$ "
                f"{soma:,.0f}), e o percentual acima usa US$ {capital:,.0f}. "
                "Numa conta compartilhada de verdade o sizing de cada trade "
                "seria menor e o P&L cairia junto; refazer isso e o replay de "
                "portfolio do 6.4, que este pacote NAO faz.",
                "",
            ]

    # --- Concentracao -------------------------------------------------------
    c: Concentracao = concentracao(trades)
    anos = " | ".join(f"{ano}: US$ {float(v):,.0f}" for ano, v in c.por_ano.items())
    linhas += [
        f"**Concentracao** -- melhor dia {c.top_dia:.0%} do net | top-5 dias "
        f"{c.top5_dias:.0%} | 2 melhores meses {c.top2_meses:.0%} | "
        f"{c.meses_positivos}/{c.meses} meses positivos | "
        f"win rate por pregao {c.wr_diario:.1f}% ({c.dias_positivos}/{c.pregoes}).",
        "",
        f"**P&L por ano civil** -- {anos}.",
        "",
    ]
    return linhas


def monta_relatorio(
    runs: list[Run],
    por: str,
    trials: list[int],
    n_boot: int,
    capital: Decimal | None = None,
) -> str:
    slippage, flatten, comissao, desconto = exige_mesma_regua(runs)
    exige_calendario(runs)
    unidades = agrupa(runs, por, capital)
    cab = [
        "<!-- gerado por trader-research (plano 5.3). Nao editar a mao. -->",
        f"**Regua dos runs:** slippage {slippage} bp/lado | flatten de fim de "
        f"sessao {flatten or 'DESLIGADO'} | comissao "
        f"{comissao or 'NAO DECLARADA (run anterior ao 5.6)'} | desconto no "
        f"alvo {desconto if desconto is not None else 'NAO DECLARADO'} bp | "
        f"{len(runs)} runs | {n_boot:,} reamostras | seed {SEED}.",
        "",
        "**Esquema do bootstrap:** estacionario (Politis-Romano) sobre o P&L "
        "diario de **todos os pregoes, zeros incluidos**, como o ADR-019 "
        "secao 8 pre-registrou. Reamostrar so os dias com trade daria outro "
        "numero e esta reportado como sensibilidade em cada bloco.",
        "",
        "Runs lidos: " + ", ".join(sorted(r.caminho.name for r in runs)) + ".",
        "",
    ]
    repetidos = simbolos_repetidos(runs)
    if repetidos:
        detalhe = "; ".join(
            f"{s} em {', '.join(sorted(e))}" for s, e in sorted(repetidos.items())
        )
        cab += [
            f"> **Ativos repetidos:** {detalhe}. Um conjunto de {len(runs)} "
            f"pares cobre {len(set(r.symbol for r in runs))} ativos distintos: "
            "as posicoes das duas estrategias caem no mesmo papel e a "
            "diversificacao e menor do que a contagem de pares sugere.",
            "",
        ]
    corpo: list[str] = []
    for u in unidades:
        corpo += bloco_unidade(u, trials, n_boot)
    return "\n".join(cab + corpo)


def main(argv: Sequence[str] | None = None) -> int:
    p = argparse.ArgumentParser(prog="trader-research", description=__doc__)
    p.add_argument("runs", nargs="+", help="JSONs do `walkforward --output`")
    p.add_argument(
        "--por",
        default="par",
        choices=("par", "estrategia", "portfolio"),
        help="recorte da analise (padrao: par)",
    )
    p.add_argument("--out", help="arquivo markdown de saida (padrao: stdout)")
    p.add_argument(
        "--trials",
        default="2,6,42",
        help="Ns do DSR, separados por virgula (padrao: 2,6,42 -- 42 e o "
        "numero de combinacoes testadas na selecao de pares de agosto)",
    )
    p.add_argument("--reamostras", type=int, default=10_000)
    p.add_argument(
        "--capital",
        help="capital de referencia do drawdown, em dolares. Sem ele, um "
        "pool usa a SOMA dos capitais dos runs -- o que nao descreve a "
        "conta real, onde as instancias dividem uma so. Obrigatorio com "
        "--por portfolio.",
    )
    p.add_argument(
        "--permitir-experimental",
        action="store_true",
        help="deixa passar runs com --set/--strategy-config (fora de gate)",
    )
    args = p.parse_args(argv)

    runs = load_runs(args.runs)
    if not args.permitir_experimental:
        exige_nao_experimental(runs)

    problemas = 0
    for r in runs:
        divs = crosscheck.confere(r.trades, r.metrics, r.initial_capital)
        if divs:
            problemas += len(divs)
            print(f"DIVERGENCIA em {r.caminho.name}:", file=sys.stderr)
            for d in divs:
                print(f"  - {d}", file=sys.stderr)
    if problemas:
        print(
            f"\n{problemas} divergencia(s) entre o que este pacote calcula e o "
            "que o motor gravou. O relatorio NAO foi gerado: um erro de "
            "leitura viraria conclusao estatistica.",
            file=sys.stderr,
        )
        return 2

    capital = Decimal(args.capital) if args.capital else None
    if args.por == "portfolio" and capital is None:
        print(
            "--por portfolio exige --capital: somar os capitais iniciais de "
            "runs de estrategias diferentes produz um drawdown percentual "
            "que nao descreve conta nenhuma. Em producao as instancias "
            "dividem uma conta (~238k, plano 2.5). O replay de portfolio "
            "com conta compartilhada e o 6.4 do plano.",
            file=sys.stderr,
        )
        return 2
    trials = sorted({int(x) for x in args.trials.split(",") if x.strip()})
    texto = monta_relatorio(runs, args.por, trials, args.reamostras, capital)
    if args.out:
        Path(args.out).write_text(texto + "\n", encoding="utf-8")
        print(f"relatorio escrito em {args.out}")
    else:
        print(texto)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
