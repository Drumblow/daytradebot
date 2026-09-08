//! Liquidez por barra — a base do cap de tamanho do ADR-020 §3.
//!
//! A conta paper enche a NBBO sem fila nem impacto, então nada no histórico
//! do projeto diz o que acontece quando a ordem é grande perto do que o ativo
//! negocia. Os números medidos: uma posição de 1× a equity (≈ US$ 238k) é
//! **82% da barra mediana de 15m em SLYV e 64% em IJS**. Os 2 bp de slippage
//! do backtest nunca foram testados nesse tamanho.
//!
//! O cap não valida nada sozinho — só o slippage real por fill (plano §5.8)
//! diz se a fração escolhida é a certa. O que ele faz é impedir que o
//! tamanho continue sendo decidido por acidente.

use rust_decimal::Decimal;
use trader_domain::Candle;

/// Notional mediano por barra: mediana de `close × volume` nas últimas
/// `lookback` barras do buffer.
///
/// **Devolve `None` quando a janela não cabe no buffer.** Fica a cargo de
/// quem chama decidir o que fazer com isso — no `RiskManager` é recusa, não
/// "segue sem cap": um teto que some quando falta dado é um teto que não
/// existe justamente no dia em que o feed falha.
///
/// A mediana (não a média) porque a distribuição por barra é assimétrica: a
/// barra de abertura e a do fechamento negociam múltiplos da barra do meio do
/// dia, e a média as deixa mandar no teto do dia inteiro.
pub fn median_bar_notional(candles: &[Candle], lookback: usize) -> Option<Decimal> {
    if lookback == 0 || candles.len() < lookback {
        return None;
    }

    let mut valores: Vec<Decimal> = candles[candles.len() - lookback..]
        .iter()
        .map(|c| (c.close * c.volume).abs())
        .collect();
    valores.sort_unstable();

    let n = valores.len();
    Some(if n % 2 == 1 {
        valores[n / 2]
    } else {
        (valores[n / 2 - 1] + valores[n / 2]) / Decimal::from(2)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use trader_domain::TimeFrame;

    fn barra(i: i64, close: i64, volume: i64) -> Candle {
        Candle::new(
            "IJS",
            TimeFrame::M15,
            Utc.timestamp_opt(1_700_000_000 + i * 900, 0).unwrap(),
            Decimal::from(close),
            Decimal::from(close),
            Decimal::from(close),
            Decimal::from(close),
            Decimal::from(volume),
        )
        .expect("candle válido")
    }

    #[test]
    fn mediana_de_janela_impar_e_o_valor_do_meio() {
        // notional: 100, 300, 200 → ordenado 100, 200, 300 → 200.
        let candles = vec![barra(0, 10, 10), barra(1, 10, 30), barra(2, 10, 20)];
        assert_eq!(
            median_bar_notional(&candles, 3),
            Some(Decimal::from(200)),
            "a mediana não pode depender da ORDEM em que as barras chegaram"
        );
    }

    #[test]
    fn mediana_de_janela_par_e_a_media_dos_dois_centrais() {
        // notional: 100, 200, 300, 400 → (200 + 300) / 2 = 250.
        let candles = vec![
            barra(0, 10, 10),
            barra(1, 10, 20),
            barra(2, 10, 30),
            barra(3, 10, 40),
        ];
        assert_eq!(median_bar_notional(&candles, 4), Some(Decimal::from(250)));
    }

    #[test]
    fn usa_apenas_as_ultimas_barras_da_janela() {
        // As três primeiras são de um mundo antigo (volume 10× maior). Com
        // lookback 3, elas não podem entrar: a janela é RECENTE por desenho,
        // senão o teto de hoje sai de uma liquidez que não existe mais.
        let candles = vec![
            barra(0, 10, 1000),
            barra(1, 10, 1000),
            barra(2, 10, 1000),
            barra(3, 10, 10),
            barra(4, 10, 30),
            barra(5, 10, 20),
        ];
        assert_eq!(median_bar_notional(&candles, 3), Some(Decimal::from(200)));
    }

    /// Falha FECHADA: janela maior que o buffer devolve `None`, e quem chama
    /// recusa. O contrário — usar o que tiver — faria o live e o backtest
    /// medirem a mediana em janelas diferentes, que é exatamente a assimetria
    /// que o ADR-020 §3 manda evitar ("fonte e N iguais são obrigatórios").
    #[test]
    fn janela_maior_que_o_buffer_nao_e_estimada() {
        let candles = vec![barra(0, 10, 10), barra(1, 10, 20)];
        assert_eq!(median_bar_notional(&candles, 3), None);
        assert_eq!(median_bar_notional(&candles, 2), Some(Decimal::from(150)));
        assert_eq!(median_bar_notional(&[], 1), None);
        assert_eq!(median_bar_notional(&candles, 0), None);
    }
}
