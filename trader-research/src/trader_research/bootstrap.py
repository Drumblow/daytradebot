"""Bootstrap estacionario (Politis e Romano, 1994) sobre a serie de pregoes.

**O esquema pre-registrado e o do ADR-019 secao 8, e ele diz "P&L diario de
TODOS os pregoes, zeros incluidos".** Isso nao e detalhe de implementacao: as
estrategias deste projeto operam em 6% a 12% dos pregoes, entao reamostrar so
os dias com trade e reamostrar 17 a 35 pontos onde o pre-registro manda
reamostrar ~270. Alem de ser outro estimador (a frequencia de trades deixa de
ser aleatoria), com n dessa ordem um bloco medio de 5 vira 20% da serie, o
bootstrap circular degenera em rotacao e o "IC95" perde cobertura. Por isso o
calendario e OBRIGATORIO para o numero que alimenta o criterio; o esquema "so
dias ativos" fica disponivel apenas como sensibilidade declarada.

**A unidade reamostrada e o pregao; a estatistica e recalculada sobre os
trades daquele pregao.** Um profit factor sobre P&L ja agregado por dia nao e
o numero do gate -- um dia com +100 e -80 entra como +20 no agregado, mas como
100/80 no PF por trade. Reamostrando o dia e somando de volta os trades dele,
o ponto central bate com o `profit_factor` do `metrics.rs` e a dependencia
serial entre pregoes fica preservada.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import date
from typing import Iterable, Sequence

import numpy as np

from .loader import Trade
from .series import et_date

#: Acima desta razao L/n o bootstrap circular degenera: a probabilidade de uma
#: reamostra nunca reiniciar bloco -- e portanto ser uma ROTACAO da serie, com
#: o mesmo total exato -- e (1-1/L)^(n-1), e o IC encolhe em vez de alargar.
#: Politis-Romano pede L/n -> 0; a regra pratica e L ~ n^(1/3).
RAZAO_BLOCO_MAXIMA = 0.10


class BlocoLongoDemais(ValueError):
    """`bloco_medio` grande demais para o tamanho da serie."""


@dataclass(frozen=True)
class PainelDiario:
    """Somas por pregao, prontas para reamostragem vetorizada."""

    dias: list
    lucro_bruto: np.ndarray
    perda_bruta: np.ndarray
    lucro_bruto_r: np.ndarray
    perda_bruta_r: np.ndarray
    soma_r: np.ndarray
    n_trades: np.ndarray
    n_ganhos: np.ndarray
    net: np.ndarray
    #: "calendario" (pre-registrado) ou "ativos" (sensibilidade).
    esquema: str

    def __len__(self) -> int:
        return len(self.dias)

    @property
    def pregoes_ativos(self) -> int:
        return int((self.n_trades > 0).sum())


def painel_diario(
    trades: Sequence[Trade], calendario: Iterable[date] | None = None
) -> PainelDiario:
    """Monta o painel por pregao.

    `calendario` = as datas ET de todos os pregoes cobertos pela amostra (o
    `oos_sessions` do JSON). Passando `None`, o painel cobre apenas os dias
    com trade -- esquema que NAO e o pre-registrado e so serve de
    sensibilidade.
    """
    agrupado: dict = {}
    for t in trades:
        agrupado.setdefault(et_date(t.exit_time), []).append(t)

    if calendario is None:
        dias = sorted(agrupado)
        esquema = "ativos"
    else:
        dias = sorted(set(calendario))
        fora = sorted(set(agrupado) - set(dias))
        if fora:
            raise ValueError(
                "ha trades fechados em datas que nao estao no calendario de "
                f"pregoes do run ({fora[:5]}): o JSON e o motor discordam "
                "sobre quais dias existiram."
            )
        esquema = "calendario"

    n = len(dias)
    gp, gl = np.zeros(n), np.zeros(n)
    gpr, glr = np.zeros(n), np.zeros(n)
    soma_r, cnt, ganhos, net = np.zeros(n), np.zeros(n), np.zeros(n), np.zeros(n)
    indice = {d: i for i, d in enumerate(dias)}
    for dia, do_dia in agrupado.items():
        i = indice[dia]
        for t in do_dia:
            pnl = float(t.net_pnl)
            r = float(t.result_in_r)
            net[i] += pnl
            soma_r[i] += r
            cnt[i] += 1
            # Mesmo criterio do `metrics.rs`: > 0 e ganho, o resto e perda
            # (zero conta como perda dos dois lados -- paridade, nao gosto).
            if pnl > 0:
                gp[i] += pnl
                ganhos[i] += 1
            else:
                gl[i] += -pnl
            if r > 0:
                gpr[i] += r
            else:
                glr[i] += -r
    return PainelDiario(dias, gp, gl, gpr, glr, soma_r, cnt, ganhos, net, esquema)


def indices_estacionarios(
    n: int,
    bloco_medio: float,
    n_reamostras: int,
    rng: np.random.Generator,
    *,
    permitir_bloco_longo: bool = False,
) -> np.ndarray:
    """Matriz (n_reamostras, n) de indices do bootstrap estacionario.

    Bloco geometrico de media `bloco_medio`: a cada passo, com probabilidade
    1/L comeca um bloco novo em posicao uniforme; senao avanca um pregao
    (circular). O tamanho ALEATORIO do bloco e o que torna a serie reamostrada
    estacionaria -- bloco fixo nao e.
    """
    if n <= 0:
        raise ValueError("serie vazia")
    if bloco_medio < 1.0:
        raise ValueError("o bloco medio precisa ser >= 1 pregao")
    if not permitir_bloco_longo and bloco_medio > RAZAO_BLOCO_MAXIMA * n:
        raise BlocoLongoDemais(
            f"bloco medio de {bloco_medio:g} sobre serie de {n} pregoes "
            f"(L/n = {bloco_medio / n:.2f}). Acima de {RAZAO_BLOCO_MAXIMA:.0%} "
            "a reamostra vira rotacao da serie com probabilidade "
            f"{(1 - 1 / bloco_medio) ** (n - 1):.1%} e o IC ENCOLHE em vez de "
            "alargar -- a cobertura fica abaixo dos 95% do rotulo. Use o "
            "calendario completo de pregoes (ADR-019 secao 8), ou "
            "permitir_bloco_longo=True se souber o que esta medindo."
        )
    p = 1.0 / bloco_medio
    idx = np.empty((n_reamostras, n), dtype=np.int64)
    idx[:, 0] = rng.integers(0, n, size=n_reamostras)
    if n > 1:
        pular = rng.random((n_reamostras, n - 1)) < p
        novos = rng.integers(0, n, size=(n_reamostras, n - 1))
        for t in range(1, n):
            continua = (idx[:, t - 1] + 1) % n
            idx[:, t] = np.where(pular[:, t - 1], novos[:, t - 1], continua)
    return idx


def indices_iid(n: int, n_reamostras: int, rng: np.random.Generator) -> np.ndarray:
    """Reamostragem iid -- o mesmo bootstrap SEM dependencia serial.

    Existe para contraste: a diferenca entre os dois ICs mede quanto do
    intervalo vinha de supor pregoes independentes.
    """
    return rng.integers(0, n, size=(n_reamostras, n))


def _razao(num: np.ndarray, den: np.ndarray) -> np.ndarray:
    seguro = np.where(den > 0, den, 1.0)
    return np.where(den > 0, num / seguro, np.inf)


@dataclass(frozen=True)
class IC:
    estatistica: str
    ponto: float
    inferior: float
    superior: float
    n_reamostras: int
    bloco_medio: float | None
    fracao_infinita: float
    esquema: str
    pregoes: int

    def linha(self) -> str:
        bloco = "iid" if self.bloco_medio is None else f"L={self.bloco_medio:g}"
        return (
            f"{self.estatistica} ({bloco}): {self.ponto:.3f} "
            f"IC95 [{self.inferior:.3f}; {self.superior:.3f}]"
        )


def _ic(valores, ponto, nome, bloco, esquema, pregoes) -> IC:
    finitos = valores[np.isfinite(valores)]
    fracao_inf = float(1.0 - finitos.size / valores.size) if valores.size else 0.0
    if finitos.size == 0:
        lo = hi = float("inf")
    else:
        # As reamostras infinitas (nenhum pregao perdedor) sao, por
        # construcao, as MAIORES da distribuicao. O quantil q da distribuicao
        # completa e o quantil q/(1-f) do subconjunto finito -- tirar o
        # percentil "cru" dos finitos devolve quantil errado nos DOIS limites.
        def q(alvo: float) -> float:
            if fracao_inf >= 1.0:
                return float("inf")
            ajustado = alvo / (1.0 - fracao_inf)
            if ajustado > 100.0:
                return float("inf")
            return float(np.percentile(finitos, ajustado))

        lo, hi = q(2.5), q(97.5)
    return IC(
        nome, ponto, lo, hi, int(valores.size), bloco, fracao_inf, esquema, pregoes
    )


def _indices(n, bloco_medio, n_reamostras, seed, permitir_bloco_longo=False):
    rng = np.random.default_rng(seed)
    if bloco_medio is None:
        return indices_iid(n, n_reamostras, rng)
    return indices_estacionarios(
        n, bloco_medio, n_reamostras, rng, permitir_bloco_longo=permitir_bloco_longo
    )


def ic_profit_factor(
    painel: PainelDiario,
    *,
    em_r: bool = False,
    bloco_medio: float | None = 5.0,
    n_reamostras: int = 10_000,
    seed: int = 20260907,
    permitir_bloco_longo: bool = False,
) -> IC:
    """IC95 percentil do profit factor. `bloco_medio=None` => iid."""
    gp = painel.lucro_bruto_r if em_r else painel.lucro_bruto
    gl = painel.perda_bruta_r if em_r else painel.perda_bruta
    nome = "PF em R" if em_r else "PF em $"
    n = len(painel)
    if n == 0:
        raise ValueError("painel diario vazio")
    ponto = float(gp.sum() / gl.sum()) if gl.sum() > 0 else float("inf")
    idx = _indices(n, bloco_medio, n_reamostras, seed, permitir_bloco_longo)
    amostras = _razao(gp[idx].sum(axis=1), gl[idx].sum(axis=1))
    return _ic(amostras, ponto, nome, bloco_medio, painel.esquema, n)


def ic_avg_r(
    painel: PainelDiario,
    *,
    bloco_medio: float | None = 5.0,
    n_reamostras: int = 10_000,
    seed: int = 20260907,
    permitir_bloco_longo: bool = False,
) -> IC:
    """IC95 do R medio por trade -- o criterio que reprova a balance-area."""
    n = len(painel)
    if n == 0:
        raise ValueError("painel diario vazio")
    ponto = float(painel.soma_r.sum() / painel.n_trades.sum())
    idx = _indices(n, bloco_medio, n_reamostras, seed, permitir_bloco_longo)
    somas = painel.soma_r[idx].sum(axis=1)
    contagens = painel.n_trades[idx].sum(axis=1)
    # Reamostra que nao pegou nenhum pregao ativo nao tem media. Com o
    # calendario completo isso e raro, mas nao impossivel, e virar 0/0 em
    # silencio puxaria o percentil.
    amostras = np.where(
        contagens > 0, somas / np.where(contagens > 0, contagens, 1.0), np.nan
    )
    validas = amostras[~np.isnan(amostras)]
    if validas.size == 0:
        raise ValueError("nenhuma reamostra pegou pregao com trade")
    return _ic(validas, ponto, "avg R", bloco_medio, painel.esquema, n)


def ic_net(
    painel: PainelDiario,
    *,
    bloco_medio: float | None = 5.0,
    n_reamostras: int = 10_000,
    seed: int = 20260907,
    permitir_bloco_longo: bool = False,
) -> IC:
    n = len(painel)
    if n == 0:
        raise ValueError("painel diario vazio")
    ponto = float(painel.net.sum())
    idx = _indices(n, bloco_medio, n_reamostras, seed, permitir_bloco_longo)
    amostras = painel.net[idx].sum(axis=1)
    return _ic(amostras, ponto, "net ($)", bloco_medio, painel.esquema, n)
