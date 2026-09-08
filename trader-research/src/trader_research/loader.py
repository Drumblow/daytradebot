"""Leitura do JSON do `trader-cli walkforward --output`.

Duas travas fecham o mesmo buraco que ja mordeu o projeto duas vezes (ADR-015
secao 4, ADR-018): juntar num mesmo numero runs medidos com reguas
diferentes. Aqui isso e erro, nao aviso.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from datetime import date, datetime
from decimal import Decimal
from pathlib import Path
from typing import Any, Iterable, Sequence

# O `walkforward` roda com o capital do `BacktestConfig`; ate a versao que
# grava `initial_capital` no JSON, este era o valor fixo do comando.
CAPITAL_PADRAO = Decimal("100000")


def _dt(texto: str) -> datetime:
    return datetime.fromisoformat(texto.replace("Z", "+00:00"))


@dataclass(frozen=True)
class Trade:
    symbol: str
    direction: str
    entry_time: datetime
    exit_time: datetime
    net_pnl: Decimal
    result_in_r: Decimal
    risk_amount: Decimal
    commissions: Decimal
    fees: Decimal
    exit_reason: str
    forced_exit: str | None

    @property
    def exit_reason_efetivo(self) -> str:
        """Espelha `Trade::effective_exit_reason` do Rust.

        Trades gravados antes do ADR-018 registram o flatten como `manual`
        com `journal.forced_exit = "session_flatten"`; sem esta traducao, o
        mesmo evento aparece em dois baldes conforme a data do run.
        """
        if self.exit_reason == "manual" and self.forced_exit == "session_flatten":
            return "end_of_day"
        return self.exit_reason

    @classmethod
    def from_json(cls, bruto: dict[str, Any]) -> "Trade":
        diario = bruto.get("journal") or {}
        forced = diario.get("forced_exit") if isinstance(diario, dict) else None
        return cls(
            symbol=bruto["symbol"],
            direction=bruto["direction"],
            entry_time=_dt(bruto["entry_time"]),
            exit_time=_dt(bruto["exit_time"]),
            net_pnl=Decimal(bruto["net_pnl"]),
            result_in_r=Decimal(bruto["result_in_r"]),
            risk_amount=Decimal(bruto["risk_amount"]),
            commissions=Decimal(bruto["commissions"]),
            fees=Decimal(bruto["fees"]),
            exit_reason=bruto["exit_reason"],
            forced_exit=forced if isinstance(forced, str) else None,
        )


@dataclass(frozen=True)
class Run:
    """Um walk-forward exportado. `trades` sao os OOS — a amostra que conta."""

    caminho: Path
    symbol: str
    strategy_id: str
    strategy_version: str
    config_hash: str
    slippage_bps: str
    session_flatten: str | None
    label: str
    experimental: bool
    overrides: list[tuple[str, str]]
    holdout_from: datetime | None
    windows: int
    initial_capital: Decimal
    #: Datas ET de TODOS os pregoes cobertos pela amostra OOS, inclusive os
    #: sem trade. `None` em run gerado antes deste campo existir -- e nesse
    #: caso o bootstrap pre-registrado do ADR-019 secao 8 NAO pode ser rodado.
    oos_sessions: list[date] | None
    trades: list[Trade]
    metrics: dict[str, Any]
    holdout_metrics: dict[str, Any] | None
    holdout_trades: list[Trade]

    @property
    def par(self) -> str:
        return f"{self.strategy_id}·{self.symbol}"

    @property
    def regua(self) -> tuple[str, str | None]:
        """O que precisa ser igual para dois runs serem somaveis."""
        return (self.slippage_bps, self.session_flatten)


def load_run(caminho: str | Path) -> Run:
    caminho = Path(caminho)
    bruto = json.loads(caminho.read_text(encoding="utf-8"))
    if "selection" not in bruto:
        raise ValueError(
            f"{caminho}: JSON sem a chave 'selection'. Isto e um export de "
            f"`backtest`, nao de `walkforward` — o bootstrap deste pacote roda "
            f"sobre a amostra OOS."
        )
    selecao = bruto["selection"]
    capital_bruto = bruto.get("initial_capital")
    return Run(
        caminho=caminho,
        symbol=bruto["symbol"],
        strategy_id=bruto["strategy_id"],
        strategy_version=bruto["strategy_version"],
        config_hash=bruto["config_hash"],
        slippage_bps=str(bruto["slippage_bps"]),
        session_flatten=bruto.get("session_flatten"),
        label=bruto.get("label") or "",
        experimental=bool(bruto.get("experimental", False)),
        overrides=[tuple(o) for o in bruto.get("overrides", [])],
        holdout_from=_dt(bruto["holdout_from"]) if bruto.get("holdout_from") else None,
        windows=int(bruto["windows"]),
        initial_capital=(
            Decimal(str(capital_bruto)) if capital_bruto is not None else CAPITAL_PADRAO
        ),
        oos_sessions=(
            [date.fromisoformat(d) for d in bruto["oos_sessions"]]
            if bruto.get("oos_sessions")
            else None
        ),
        trades=[Trade.from_json(t) for t in selecao["oos_trades"]],
        metrics=selecao["oos_metrics"],
        holdout_metrics=bruto.get("holdout"),
        holdout_trades=[Trade.from_json(t) for t in bruto.get("holdout_trades") or []],
    )


def load_runs(caminhos: Iterable[str | Path]) -> list[Run]:
    caminhos = [Path(c) for c in caminhos]
    resolvidos: dict[Path, Path] = {}
    for c in caminhos:
        chave = c.resolve()
        if chave in resolvidos:
            raise ValueError(
                f"o mesmo arquivo entrou duas vezes ({c.name}). Somar um run "
                "com ele mesmo dobra trades, P&L e amostra sem nenhum aviso."
            )
        resolvidos[chave] = c
    runs = [load_run(c) for c in caminhos]

    vistos: dict[tuple, Path] = {}
    for r in runs:
        chave = (
            r.strategy_id,
            r.symbol,
            r.config_hash,
            r.slippage_bps,
            r.session_flatten,
        )
        if chave in vistos:
            raise ValueError(
                f"dois runs descrevem a mesma combinacao {r.strategy_id} x "
                f"{r.symbol} com o mesmo config_hash e a mesma regua "
                f"({vistos[chave].name} e {r.caminho.name}). Um deles e copia."
            )
        vistos[chave] = r.caminho
    return runs


class SemCalendario(ValueError):
    """Run sem `oos_sessions` -- o esquema pre-registrado nao roda."""


def exige_calendario(runs: Sequence[Run]) -> None:
    """Falha fechado se algum run nao trouxer o calendario de pregoes.

    O ADR-019 secao 8 pre-registra o bootstrap sobre "o P&L diario de todos os
    pregoes, ZEROS INCLUIDOS". Sem a lista de pregoes nao da para saber quais
    dias tiveram zero, e o que sobra e outro estimador: reamostrar so os 17-35
    dias com trade em vez dos ~270 do periodo. Rodar isso e chamar de
    "criterio do ADR-019 secao 7" seria trocar o esquema pre-registrado em
    silencio -- exatamente o que o pre-registro existe para impedir.
    """
    velhos = [r for r in runs if not r.oos_sessions]
    if velhos:
        nomes = ", ".join(r.caminho.name for r in velhos)
        raise SemCalendario(
            f"sem `oos_sessions`: {nomes}. Estes runs sao anteriores ao campo "
            "e nao permitem o bootstrap pre-registrado (ADR-019 secao 8). "
            "Rode o walkforward de novo com o binario atual."
        )


def simbolos_repetidos(runs: Sequence[Run]) -> dict[str, list[str]]:
    """Simbolos que aparecem em mais de um run do conjunto.

    Um "portfolio de 8 pares" com AVUV em duas estrategias tem 7 ativos, nao
    8: as duas posicoes se sobrepoem no mesmo papel e a diversificacao e menor
    do que a contagem de pares sugere.
    """
    por_simbolo: dict[str, list[str]] = {}
    for r in runs:
        por_simbolo.setdefault(r.symbol, []).append(r.strategy_id)
    return {s: e for s, e in por_simbolo.items() if len(e) > 1}


class ReguaDivergente(ValueError):
    """Runs medidos com custo ou regra de fim de sessao diferentes."""


def exige_mesma_regua(runs: Sequence[Run]) -> tuple[str, str | None]:
    """Falha fechado se os runs nao forem somaveis.

    Precedente: o ADR-015 secao 4 e o ADR-018 invalidaram *todos* os runs
    anteriores porque o motor mudou de regra. Somar um run com flatten e um
    sem produz um numero que nao descreve mundo nenhum.
    """
    if not runs:
        raise ValueError("nenhum run informado")
    reguas = {r.regua for r in runs}
    if len(reguas) > 1:
        detalhe = "; ".join(
            f"{r.caminho.name}: slippage={r.slippage_bps} bp, flatten={r.session_flatten}"
            for r in runs
        )
        raise ReguaDivergente(
            "runs medidos com reguas diferentes nao sao somaveis — " + detalhe
        )
    return runs[0].regua


def exige_nao_experimental(runs: Sequence[Run]) -> None:
    """Ablacao (`--set`/`--strategy-config`) nao entra em relatorio de gate."""
    sujos = [r for r in runs if r.experimental]
    if sujos:
        nomes = ", ".join(f"{r.caminho.name} (label={r.label!r})" for r in sujos)
        raise ValueError(
            "runs experimentais nao podem alimentar um relatorio de gate: " + nomes
        )
