"""Confere, trade a trade, se este pacote le o run do mesmo jeito que o motor.

Sem isto, um erro de encanamento (campo errado, fuso errado, sinal invertido)
sai como conclusao estatistica bonita. O metodo e o unico que vale: recalcular
em Python o que o `metrics.rs` ja calculou em Rust, a partir dos MESMOS trades
do JSON, e exigir igualdade. Divergencia e erro de um dos dois lados -- e a
mensagem diz de qual campo.

Um campo do motor fica DE FORA de proposito: `sharpe_ratio`. Ele anualiza pelo
intervalo mediano da serie de equity e, por candle de 15 min, produz Sharpe de
-6 a -9 com PF > 1 (item 5 das pendencias do ADR-019). Compara-lo exigiria
reproduzir o defeito. O Sharpe deste pacote e outro numero, calculado na
frequencia da propria serie, e por isso nao entra no crosscheck.

Todo o resto entra: os escalares e os TRES mapas por grupo. O
`by_entry_hour_et` importa mais do que parece — e a unica metrica do motor
derivada de `entry_time`, entao sem ela trocar entrada por saida em qualquer
lugar deste pacote passaria despercebido nesta amostra (nenhum dos 214 trades
reais atravessa a noite).
"""

from __future__ import annotations

from dataclasses import dataclass
from decimal import Decimal
from typing import Any, Sequence

import numpy as np

from .concentration import concentracao
from .loader import Trade
from .series import NY, vetor_r, vetor_risco

TOL_RELATIVA = Decimal("1e-12")
TOL_FLOAT = 1e-9
AUSENTE = "<campo ausente no JSON>"


@dataclass(frozen=True)
class Divergencia:
    campo: str
    rust: Any
    python: Any

    def __str__(self) -> str:
        return f"{self.campo}: rust={self.rust} python={self.python}"


def _perto(a: Decimal, b: Decimal) -> bool:
    if a == b:
        return True
    escala = max(abs(a), abs(b), Decimal(1))
    return abs(a - b) / escala <= TOL_RELATIVA


def t_stat(xs: np.ndarray) -> float:
    """Espelha `t_stat` do metrics.rs: media / (desvio amostral / sqrt(n))."""
    if xs.size < 2:
        return 0.0
    desvio = float(np.std(xs, ddof=1))
    if desvio == 0.0:
        return 0.0
    return float(np.mean(xs)) / (desvio / np.sqrt(xs.size))


def correlacao(a: np.ndarray, b: np.ndarray) -> float:
    """Espelha `correlacao` do metrics.rs (Pearson)."""
    if a.size < 2 or a.size != b.size:
        return 0.0
    da, db = a - a.mean(), b - b.mean()
    den = np.sqrt((da**2).sum()) * np.sqrt((db**2).sum())
    if den == 0.0:
        return 0.0
    return float((da * db).sum() / den)


def confere(
    trades: Sequence[Trade], metrics: dict[str, Any], capital: Decimal
) -> list[Divergencia]:
    divergencias: list[Divergencia] = []

    def cmp_dec(campo: str, calculado: Decimal | None) -> None:
        # `None` no JSON e legitimo (PF sem perdas); campo AUSENTE nao e --
        # significa export truncado ou de outra versao, e passar batido faria
        # o crosscheck aprovar um arquivo que ele nunca conferiu.
        if campo not in metrics:
            divergencias.append(Divergencia(campo, AUSENTE, calculado))
            return
        bruto = metrics[campo]
        esperado = None if bruto is None else Decimal(str(bruto))
        if esperado is None and calculado is None:
            return
        if esperado is None or calculado is None or not _perto(esperado, calculado):
            divergencias.append(Divergencia(campo, esperado, calculado))

    def cmp_int(campo: str, calculado: int) -> None:
        if campo not in metrics:
            divergencias.append(Divergencia(campo, AUSENTE, calculado))
            return
        if int(metrics[campo]) != calculado:
            divergencias.append(Divergencia(campo, metrics[campo], calculado))

    def cmp_float(campo: str, calculado: float) -> None:
        if campo not in metrics:
            divergencias.append(Divergencia(campo, AUSENTE, calculado))
            return
        esperado = float(metrics[campo])
        if abs(esperado - calculado) > TOL_FLOAT:
            divergencias.append(Divergencia(campo, esperado, calculado))

    n = len(trades)
    cmp_int("total_trades", n)
    ganhos = [t for t in trades if t.net_pnl > 0]
    cmp_int("winning_trades", len(ganhos))
    cmp_int("losing_trades", n - len(ganhos))

    gp = sum((t.net_pnl for t in trades if t.net_pnl > 0), Decimal(0))
    gl = sum((-t.net_pnl for t in trades if t.net_pnl <= 0), Decimal(0))
    cmp_dec("gross_profit", gp)
    cmp_dec("gross_loss", gl)
    cmp_dec("net_pnl", gp - gl)
    cmp_dec("profit_factor", (gp / gl) if gl != 0 else None)

    gpr = sum((t.result_in_r for t in trades if t.result_in_r > 0), Decimal(0))
    glr = sum((-t.result_in_r for t in trades if t.result_in_r <= 0), Decimal(0))
    cmp_dec("profit_factor_r", (gpr / glr) if glr != 0 else None)
    cmp_dec("avg_r_per_trade", sum((t.result_in_r for t in trades), Decimal(0)) / n)
    cmp_dec("cost_total", sum((t.commissions + t.fees for t in trades), Decimal(0)))

    # Drawdown com a convencao do motor: equity comeca no capital inicial e o
    # percentual e sobre o PICO.
    equity = capital
    pico = capital
    dd_pct = Decimal(0)
    dd_abs = Decimal(0)
    for t in trades:
        equity += t.net_pnl
        pico = max(pico, equity)
        queda = pico - equity
        if queda > dd_abs:
            dd_abs = queda
            dd_pct = Decimal(0) if pico == 0 else queda / pico * Decimal(100)
    cmp_dec("max_drawdown", dd_abs)
    cmp_dec("max_drawdown_pct", dd_pct)

    c = concentracao(trades)
    cmp_int("trading_days", c.pregoes)
    cmp_int("months_total", c.meses)
    cmp_int("months_positive", c.meses_positivos)
    cmp_float("top_day_share", c.top_dia)
    cmp_float("top5_day_share", c.top5_dias)
    cmp_float("top2_month_share", c.top2_meses)

    cmp_float("t_stat_avg_r", t_stat(vetor_r(trades)))
    cmp_float("corr_risk_result", correlacao(vetor_risco(trades), vetor_r(trades)))

    cmp_dec("win_rate", Decimal(len(ganhos)) / Decimal(n) * Decimal(100))
    cmp_dec("avg_pnl_per_trade", (gp - gl) / n)
    cmp_dec("best_trade", max(t.net_pnl for t in trades))
    cmp_dec("worst_trade", min(t.net_pnl for t in trades))

    # Maior sequencia de perdas: e a unica checagem que depende da ORDEM da
    # lista de trades. Sem ela, ler os trades fora de ordem passaria batido.
    pior = corrente = 0
    for t in trades:
        if t.net_pnl > 0:
            corrente = 0
        else:
            corrente += 1
            pior = max(pior, corrente)
    cmp_int("max_consecutive_losses", pior)

    # Os tres mapas por grupo. `by_entry_hour_et` e a UNICA metrica do motor
    # derivada de `entry_time`: sem conferi-la, trocar entry por exit em
    # qualquer lugar deste pacote passaria despercebido.
    divergencias += _confere_grupos(
        "by_exit_reason",
        metrics,
        _agrupa(trades, lambda t: t.exit_reason_efetivo),
    )
    divergencias += _confere_grupos(
        "by_direction", metrics, _agrupa(trades, lambda t: t.direction)
    )
    divergencias += _confere_grupos(
        "by_entry_hour_et",
        metrics,
        _agrupa(trades, lambda t: f"{t.entry_time.astimezone(NY).hour:02d}"),
    )

    return divergencias


def _agrupa(trades: Sequence[Trade], chave) -> dict[str, list[Trade]]:
    grupos: dict[str, list[Trade]] = {}
    for t in trades:
        grupos.setdefault(chave(t), []).append(t)
    return grupos


def _confere_grupos(
    campo: str, metrics: dict[str, Any], grupos: dict[str, list[Trade]]
) -> list[Divergencia]:
    """Compara um mapa `GroupMetrics` do motor com o mesmo mapa em Python."""
    saida: list[Divergencia] = []
    if campo not in metrics:
        return [Divergencia(campo, AUSENTE, sorted(grupos))]
    do_motor = metrics[campo] or {}
    if set(do_motor) != set(grupos):
        return [Divergencia(f"{campo} (chaves)", sorted(do_motor), sorted(grupos))]
    for chave, do_grupo in sorted(grupos.items()):
        ref = do_motor[chave]
        n = len(do_grupo)
        gp = sum((t.net_pnl for t in do_grupo if t.net_pnl > 0), Decimal(0))
        gl = sum((-t.net_pnl for t in do_grupo if t.net_pnl <= 0), Decimal(0))
        esperados = {
            "trades": (Decimal(ref["trades"]), Decimal(n)),
            "wins": (
                Decimal(ref["wins"]),
                Decimal(sum(1 for t in do_grupo if t.net_pnl > 0)),
            ),
            "net_pnl": (Decimal(str(ref["net_pnl"])), gp - gl),
            "gross_profit": (Decimal(str(ref["gross_profit"])), gp),
            "gross_loss": (Decimal(str(ref["gross_loss"])), gl),
            "avg_r": (
                Decimal(str(ref["avg_r"])),
                sum((t.result_in_r for t in do_grupo), Decimal(0)) / n,
            ),
        }
        for nome, (a, b) in esperados.items():
            if not _perto(a, b):
                saida.append(Divergencia(f"{campo}[{chave}].{nome}", a, b))
        pf_ref = ref.get("profit_factor")
        pf_py = (gp / gl) if gl != 0 else None
        if (pf_ref is None) != (pf_py is None) or (
            pf_ref is not None and not _perto(Decimal(str(pf_ref)), pf_py)
        ):
            saida.append(Divergencia(f"{campo}[{chave}].profit_factor", pf_ref, pf_py))
    return saida
