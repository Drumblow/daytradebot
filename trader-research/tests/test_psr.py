from __future__ import annotations

import math

import numpy as np
import pytest

from trader_research import psr


def test_sharpe_amostral_usa_desvio_amostral():
    x = np.array([1.0, 2.0, 3.0, 4.0])
    esperado = float(np.mean(x)) / float(np.std(x, ddof=1))
    assert psr.sharpe_amostral(x) == pytest.approx(esperado)


def test_serie_constante_nao_produz_sharpe():
    assert math.isnan(psr.sharpe_amostral(np.array([2.0, 2.0, 2.0])))


def test_curtose_e_bruta_nao_excedente():
    rng = np.random.default_rng(0)
    x = rng.normal(size=200_000)
    _, g4 = psr.skew_kurt(x)
    # Normal tem curtose bruta 3 (excedente 0). A formula do PSR usa (g4-1)/4,
    # que so esta certa com a curtose BRUTA.
    assert g4 == pytest.approx(3.0, abs=0.1)


def test_psr_de_serie_normal_com_sharpe_positivo_cresce_com_n():
    rng = np.random.default_rng(1)
    def p(n):
        x = rng.normal(loc=0.1, scale=1.0, size=n)
        sr = psr.sharpe_amostral(x)
        g3, g4 = psr.skew_kurt(x)
        return psr.psr(sr, n, g3, g4)
    # Mais observacoes com o mesmo edge => mais confianca de que SR > 0.
    assert p(2000) > p(60)


def test_psr_e_meio_quando_o_sharpe_iguala_a_referencia():
    x = np.array([1.0, -1.0, 2.0, -2.0, 3.0, 0.5, -0.7, 1.1])
    sr = psr.sharpe_amostral(x)
    g3, g4 = psr.skew_kurt(x)
    assert psr.psr(sr, x.size, g3, g4, sr_referencia=sr) == pytest.approx(0.5)


def test_sr_estrela_cresce_com_o_numero_de_tentativas():
    var = 0.01
    valores = [psr.sr_esperado_maximo(var, n) for n in (2, 5, 20, 100)]
    assert valores == sorted(valores)
    assert all(v > 0 for v in valores)


def test_dsr_cai_quando_o_numero_de_tentativas_sobe():
    x = np.random.default_rng(3).normal(loc=0.15, scale=1.0, size=120)
    res = psr.analisa(x, [2, 6, 42])
    assert res.dsr_por_trials[2] > res.dsr_por_trials[6] > res.dsr_por_trials[42]
    lo, hi = res.faixa_dsr
    assert lo == res.dsr_por_trials[42] and hi == res.dsr_por_trials[2]


def test_dsr_com_uma_unica_tentativa_e_erro():
    with pytest.raises(ValueError):
        psr.sr_esperado_maximo(0.01, 1)


def test_dsr_e_sempre_menor_que_o_psr():
    x = np.random.default_rng(4).normal(loc=0.2, scale=1.0, size=90)
    res = psr.analisa(x, [2, 42])
    assert all(v < res.psr_zero for v in res.dsr_por_trials.values())


def test_variancia_do_sharpe_encolhe_com_a_amostra():
    assert psr.variancia_sharpe(0.1, 1000, 0.0, 3.0) < psr.variancia_sharpe(
        0.1, 50, 0.0, 3.0
    )


def test_gamma_de_euler_esta_correta():
    # Valor de referencia; um erro aqui desloca todo o SR* do DSR.
    assert psr.GAMMA_EULER == pytest.approx(0.5772156649, abs=1e-9)


# ---------------------------------------------------------------------------
# Testes de VALOR, nao de propriedade.
#
# A revisao adversarial de 08/09/2026 mostrou que os testes acima sao todos
# relacionais (monotonicidade, ordem, NaN) e que quatro mutacoes do nucleo do
# PSR passavam a suite inteira: trocar o sinal do termo de assimetria, usar
# curtose excedente no lugar da bruta, apagar o termo em SR^2 e dividir por n
# em vez de n-1. Nenhuma delas quebrava um teste, e todas mudavam o numero
# impresso no relatorio. Como o `metrics.rs` nao tem PSR, o crosscheck contra
# o motor nao cobre nada disto: esta suite e a unica rede.
#
# As constantes abaixo foram calculadas fora do codigo, a partir da formula
# publicada (Bailey & Lopez de Prado 2012; Mertens 2002).
# ---------------------------------------------------------------------------


def test_variancia_sharpe_caso_normal_fixa_curtose_e_denominador():
    # Serie normal: g3=0, g4=3 (BRUTA). V = (1 + SR^2/2)/(n-1).
    # SR=0.5, n=101 -> (1 + 0.125)/100 = 0.01125.
    # Este unico valor prende de uma vez: o coeficiente (g4-1)/4, o uso da
    # curtose bruta e o n-1.
    assert psr.variancia_sharpe(0.5, 101, 0.0, 3.0) == pytest.approx(0.01125, abs=1e-12)


def test_variancia_sharpe_com_assimetria_prende_o_SINAL():
    # SR=0.4, n=51, g3=1.5, g4=6:
    #   (1 - 1.5*0.4 + (6-1)/4*0.16)/50 = (1 - 0.6 + 0.2)/50 = 0.012
    # Assimetria POSITIVA reduz V[SR]. Com `+ g3*SR` daria (1+0.6+0.2)/50 =
    # 0.036, tres vezes maior.
    assert psr.variancia_sharpe(0.4, 51, 1.5, 6.0) == pytest.approx(0.012, abs=1e-12)


def test_assimetria_positiva_reduz_a_variancia_do_sharpe():
    simetrica = psr.variancia_sharpe(0.4, 51, 0.0, 6.0)
    positiva = psr.variancia_sharpe(0.4, 51, 1.5, 6.0)
    negativa = psr.variancia_sharpe(0.4, 51, -1.5, 6.0)
    assert positiva < simetrica < negativa


def test_psr_valor_fechado():
    # SR=0.2, n=101, normal: V = 0.0102, z = 0.2/sqrt(0.0102) = 1.98037...,
    # Phi(z) = 0.9761648096719193.
    assert psr.psr(0.2, 101, 0.0, 3.0) == pytest.approx(0.9761648096719193, abs=1e-12)


def test_psr_com_referencia_desloca_o_z():
    # Mesmo caso, referencia 0.1: z = 0.1/sqrt(0.0102).
    esperado = psr._N.cdf(0.1 / math.sqrt(0.0102))
    assert psr.psr(0.2, 101, 0.0, 3.0, sr_referencia=0.1) == pytest.approx(
        esperado, abs=1e-12
    )
    assert psr.psr(0.2, 101, 0.0, 3.0, sr_referencia=0.1) < psr.psr(0.2, 101, 0.0, 3.0)


def test_sr_esperado_maximo_valor_fechado():
    # V=0.01 (sqrt=0.1), N=10:
    #   Phi^-1(1 - 1/10)      = 1.2815515655446008
    #   Phi^-1(1 - 1/(10*e))  = 1.7892417645816279
    #   SR* = 0.1 * ((1-g)*1.28155... + g*1.78924...) = 0.157459830134575
    # Prende os dois pesos de gamma, os dois argumentos e o sqrt(V).
    assert psr.sr_esperado_maximo(0.01, 10) == pytest.approx(
        0.157459830134575, abs=1e-12
    )


def test_sr_esperado_maximo_escala_com_a_raiz_da_variancia():
    # Se o sqrt sumisse da formula, dobrar V dobraria SR* em vez de
    # multiplica-lo por sqrt(2).
    a = psr.sr_esperado_maximo(0.01, 10)
    b = psr.sr_esperado_maximo(0.02, 10)
    assert b / a == pytest.approx(math.sqrt(2.0), abs=1e-12)


def test_dsr_valor_fechado():
    # DSR = PSR(SR*) com o SR* acima. Com SR=0.5, n=101, normal:
    # V=0.01125, SR*(V, N=10) calculado da mesma V.
    res = psr.analisa(_serie_com(sr=0.5, n=101), [10])
    estrela = psr.sr_esperado_maximo(res.var_sharpe, 10)
    assert res.sr_estrela_por_trials[10] == pytest.approx(estrela, abs=1e-15)
    assert res.dsr_por_trials[10] == pytest.approx(
        psr.psr(res.sharpe, res.n, res.skew, res.kurtose, sr_referencia=estrela),
        abs=1e-15,
    )


def _serie_com(sr: float, n: int) -> np.ndarray:
    """Serie determinista com Sharpe amostral exatamente `sr`."""
    base = np.array([(-1.0) ** i for i in range(n)], dtype=float)
    base = base - base.mean()
    base = base / np.std(base, ddof=1)
    return base + sr
