"""Probabilistic Sharpe Ratio e Deflated Sharpe Ratio (Bailey e Lopez de Prado).

Duas honestidades que o relatorio precisa carregar por escrito:

1. **O PSR e sobre a serie de P&L DIARIO**, nao sobre a serie de R por trade.
   Sao numeros diferentes: o R por trade ignora quantos trades cabem num dia
   e trata cada trade como uma observacao independente, o que superestima a
   amostra quando dois trades fecham no mesmo pregao. As duas versoes sao
   calculadas e reportadas lado a lado justamente porque divergem.

2. **A direcao do erro do DSR e DESCONHECIDA** -- e uma versao anterior deste
   arquivo afirmava o contrario. O DSR exige V[{SR_n}], a variancia dos
   Sharpes ENTRE as tentativas testadas; o projeto nao registra tentativas
   (`n_trials` e o item 3 da lista de pendencias do ADR-019). Na falta dela
   usamos V[SR^], a variancia do ESTIMADOR do Sharpe de UMA serie. As duas
   nao tem relacao fixa:

   - tentativas quase INDEPENDENTES do mesmo tamanho => V[{SR_n}] ~ V[SR^], e
     o DSR impresso e aproximadamente o correto;
   - tentativas CORRELACIONADAS -- variacoes de parametro da mesma regra,
     sobre os mesmos ativos, no mesmo periodo, que e o caso registrado deste
     projeto => V[{SR_n}] < V[SR^], SR* menor, e o DSR impresso e
     PESSIMISTA (o real e melhor);
   - tentativas cobrindo estrategias genuinamente diferentes => V[{SR_n}] >
     V[SR^], e so entao o DSR impresso e otimista.

   Ou seja: enquanto o item 3 do ADR-019 nao existir, o DSR daqui e uma
   substituicao declarada, nao um limite em direcao conhecida.
"""

from __future__ import annotations

import math
from dataclasses import dataclass
from statistics import NormalDist

import numpy as np

_N = NormalDist()
GAMMA_EULER = 0.5772156649015329


def sharpe_amostral(x: np.ndarray) -> float:
    """Sharpe na frequencia da propria serie (sem anualizar).

    Nao anualiza de proposito: o PSR usa SR e n na MESMA frequencia, e foi
    exatamente o anualizador por candle de 15 min que produziu os Sharpe de
    -6 a -9 com PF > 1 no `metrics.rs` (ADR-019, contexto).
    """
    if x.size < 2:
        return float("nan")
    desvio = float(np.std(x, ddof=1))
    if desvio == 0.0:
        return float("nan")
    return float(np.mean(x)) / desvio


def skew_kurt(x: np.ndarray) -> tuple[float, float]:
    """Assimetria e curtose NAO-EXCEDENTE (a formula do PSR usa gamma4 bruto)."""
    n = x.size
    if n < 2:
        return (float("nan"), float("nan"))
    desvio = float(np.std(x, ddof=1))
    if desvio == 0.0:
        return (float("nan"), float("nan"))
    centrado = x - float(np.mean(x))
    g3 = float(np.mean(centrado**3)) / desvio**3
    g4 = float(np.mean(centrado**4)) / desvio**4
    return (g3, g4)


def variancia_sharpe(sr: float, n: int, g3: float, g4: float) -> float:
    """V[SR] do estimador (Mertens/Bailey): (1 - g3*SR + (g4-1)/4*SR^2)/(n-1)."""
    if n < 2:
        return float("nan")
    return (1.0 - g3 * sr + (g4 - 1.0) / 4.0 * sr * sr) / (n - 1)


def psr(sr: float, n: int, g3: float, g4: float, sr_referencia: float = 0.0) -> float:
    """P(SR verdadeiro > sr_referencia) dado o SR observado."""
    var = variancia_sharpe(sr, n, g3, g4)
    if not math.isfinite(var) or var <= 0.0:
        return float("nan")
    return _N.cdf((sr - sr_referencia) / math.sqrt(var))


def sr_esperado_maximo(var_sr: float, n_trials: int) -> float:
    """E[max SR] sob a hipotese nula de nenhum edge, para `n_trials` tentativas.

    SR* = sqrt(V[SR]) * [ (1-g)*Phi^-1(1 - 1/N) + g*Phi^-1(1 - 1/(N*e)) ]
    """
    if n_trials < 2:
        raise ValueError("o DSR so faz sentido com N >= 2 tentativas")
    if not math.isfinite(var_sr) or var_sr < 0.0:
        return float("nan")
    a = _N.inv_cdf(1.0 - 1.0 / n_trials)
    b = _N.inv_cdf(1.0 - 1.0 / (n_trials * math.e))
    return math.sqrt(var_sr) * ((1.0 - GAMMA_EULER) * a + GAMMA_EULER * b)


@dataclass(frozen=True)
class ResultadoPSR:
    n: int
    sharpe: float
    skew: float
    kurtose: float
    psr_zero: float
    var_sharpe: float
    dsr_por_trials: dict[int, float]
    sr_estrela_por_trials: dict[int, float]

    @property
    def faixa_dsr(self) -> tuple[float, float]:
        """DSR como FAIXA — nunca pass/fail (plano secao 5.3)."""
        vals = list(self.dsr_por_trials.values())
        return (min(vals), max(vals)) if vals else (float("nan"), float("nan"))


def analisa(x: np.ndarray, trials: list[int]) -> ResultadoPSR:
    sr = sharpe_amostral(x)
    g3, g4 = skew_kurt(x)
    var = variancia_sharpe(sr, x.size, g3, g4)
    estrelas = {n: sr_esperado_maximo(var, n) for n in trials}
    dsrs = {n: psr(sr, x.size, g3, g4, sr_referencia=e) for n, e in estrelas.items()}
    return ResultadoPSR(
        n=int(x.size),
        sharpe=sr,
        skew=g3,
        kurtose=g4,
        psr_zero=psr(sr, x.size, g3, g4, 0.0),
        var_sharpe=var,
        dsr_por_trials=dsrs,
        sr_estrela_por_trials=estrelas,
    )
