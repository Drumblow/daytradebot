# `sql/stats/` — estatísticas do banco de candles (não são screens)

**Status:** ferramentas de pesquisa trazidas do scratchpad da pesquisa multiagente de
06–07/09/2026 (leitor de banco). Nada aqui simula trade; são as consultas que
alimentaram o mapa do banco e o plano-mestre (`docs/cto-plano-lucratividade-2026-09.md`
§2.2, §2.4, §2.3 achado 7). Para screens com fill honesto, ver `sql/screens/README.md`.

## Como executar

```bash
docker exec -i trader-postgres psql -U trader -d trader_db < sql/stats/01-liquidez-diaria.sql
```

(PowerShell: `Get-Content <arquivo> | docker exec -i trader-postgres psql -U trader -d trader_db`.)
Só `candles` e `assets`; fuso **America/New_York** em toda agregação por dia e por hora;
sem tabelas temporárias — os arquivos podem ser concatenados numa sessão só.

Ressalvas: 6 dos 14 símbolos (IJR, MDY, QQQ, SCHA, SPY, VB) param em 06/08/2026 no banco
dev (plano §5.6) — médias "por símbolo" misturam períodos; alguns arquivos têm filtros de
data fixos (ago/2026, mai–jul/2026) porque foram escritos para responder perguntas
daquela semana — ajustar antes de reutilizar. `SPY`, `QQQ` e `MDY` são excluídos onde a
pergunta era sobre small caps.

## Arquivos

| Arquivo | Pergunta que responde | Onde o plano usa |
|---|---|---|
| `01-liquidez-diaria.sql` | Volume, US$ negociados, preço médio e range diário (média, mediana, p10) por símbolo | §2.4 (cap por liquidez), §5.5 |
| `02-range-por-barra.sql` | Range e corpo médios da barra de 15m, US$ por barra, % de barras com volume zero | §2.2 (stop ≥ 1 range médio de barra), §5.9 (IWV: 0,8% das barras sem volume) |
| `03-liquidez-por-barra-e-hora.sql` | US$ por barra às 09:30, no meio do dia (11h–14h) e às 15:45; que fração de uma barra mediana de meio-dia é uma posição de US$ 250k; p10 | §2.4: SLYV/IJS com 82%/64% de uma barra mediana |
| `04-range-diario-por-mes.sql` | Range diário médio e retorno intradiário acumulado por mês (7 símbolos) | §2.2: ago/2026 = 0,78%, abr/2025 = 3,43% |
| `05-gaps-de-abertura.sql` | Gap de abertura (média, p50, p90), % de gaps preenchidos, correlação gap × intradia | §8 (kill pré-registrado de IBIT/ETHA por gap), screen 04 |
| `06-autocorrelacao-por-hora.sql` | ACF(1) dos retornos de 15m por hora do dia (Pearson e Spearman; 2025 vs 2026; % de reversão) | §2.1 (o que ganha é reversão à média) |
| `07-autocorrelacao-por-simbolo.sql` | ACF(1)/ACF(2) intradiário por símbolo; ACF diária e intradiária robusta (ex mar–abr/2025, 2026), gap × intradia | idem |
| `08-proxy-de-tendencia.sql` | Proxy de `trend_state` (close/SMA20/SMA50): % up/down/neutral por símbolo e por mês | §2.3 achado 5, §6.1 (39–42% das barras neutral) |
| `09-initial-balance.sql` | Quanto do range diário a 1ª barra e a 1ª hora contêm; em quantos dias a 1ª hora faz a máxima/mínima do dia; rompimento de ambos os lados | screen 03, §6.6 (`day_type`) |
| `10-distribuicao-range-diario.sql` | % de dias com range < 1% / < 0,75% / > 2%; range médio por período (ago/26, mai–jul/26, 2025) | §2.2, §5.9 (IWV: 61% dos dias < 1%) |
| `11-perfil-intradiario.sql` | Range, corpo, US$ e retorno médios por barra do dia; gap entre barras consecutivas (p99) e % de barras com |retorno| > 1%/2% | §5.8 (o que uma barra de 15m esconde) |
| `12-barra-1h-range.sql` | Range mediano de uma barra de 1h derivada do 15m vs a barra de 15m (proxy de stop para o timeframe 1h) | §7 (timeframe 1h: stop mediano 0,35% vs 0,21%) |

Os números citados na coluna "Onde o plano usa" são os do plano-mestre e da re-simulação
dos críticos (06/09/2026); as consultas produzem os valores atuais do banco, que mudam a
cada ingestão.
