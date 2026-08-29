/* Painel do HumanStyle Trader Bot.
 *
 * Vanilla JS, sem libs: os gráficos são SVG gerado à mão. Todo texto vindo do
 * banco entra no DOM via textContent — nunca innerHTML com dado externo. */

"use strict";

const REFRESH_MS = 30000;

const $ = (id) => document.getElementById(id);

// ── Formatação ──────────────────────────────────────────────────────────────

const moneyFmt = new Intl.NumberFormat("en-US", {
  style: "currency", currency: "USD", maximumFractionDigits: 2,
});

// Rótulos de eixo: sem centavos, para caber na margem esquerda dos gráficos.
const axisMoneyFmt = new Intl.NumberFormat("en-US", {
  style: "currency", currency: "USD", maximumFractionDigits: 0,
});

// "2026-08-04" (date do Postgres) → "04/08/26", sem sofrer com timezone.
function fmtDay(isoDay) {
  const [y, m, d] = String(isoDay).split("-");
  return d && m && y ? `${d}/${m}/${y.slice(2)}` : String(isoDay);
}

function fmtMoney(v) {
  const n = Number(v);
  if (!isFinite(n)) return "—";
  return (n > 0 ? "+" : "") + moneyFmt.format(n);
}

const etDateTime = new Intl.DateTimeFormat("pt-BR", {
  timeZone: "America/New_York",
  day: "2-digit", month: "2-digit", hour: "2-digit", minute: "2-digit",
});
const etDate = new Intl.DateTimeFormat("pt-BR", {
  timeZone: "America/New_York", day: "2-digit", month: "2-digit", year: "2-digit",
});

function fmtET(iso) {
  if (!iso) return "—";
  return etDateTime.format(new Date(iso));
}

function fmtAge(iso) {
  if (!iso) return "nunca";
  const s = (Date.now() - new Date(iso).getTime()) / 1000;
  if (s < 90) return "agora há pouco";
  if (s < 3600) return `há ${Math.round(s / 60)} min`;
  if (s < 86400) return `há ${Math.round(s / 3600)} h`;
  return `há ${Math.round(s / 86400)} d`;
}

function fmtR(v) {
  const n = Number(v);
  if (!isFinite(n)) return "—";
  return (n > 0 ? "+" : "") + n.toFixed(2) + "R";
}

function fmtNum(v, digits = 2) {
  const n = Number(v);
  return isFinite(n) ? n.toFixed(digits) : "—";
}

// ── Helpers de DOM ──────────────────────────────────────────────────────────

function el(tag, attrs = {}, children = []) {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === "text") node.textContent = v;
    else if (k === "class") node.className = v;
    else node.setAttribute(k, v);
  }
  for (const child of children) node.appendChild(child);
  return node;
}

function chip(text, kind) {
  return el("span", { class: `chip chip-${kind}`, text });
}

function td(text, cls) {
  const c = el("td", { text: text ?? "—" });
  if (cls) c.className = cls;
  return c;
}

function tdChip(text, kind) {
  const c = el("td");
  if (text) c.appendChild(chip(text, kind));
  else c.textContent = "—";
  return c;
}

function pnlTd(v) {
  const n = Number(v);
  return td(fmtMoney(v), "num " + (n > 0 ? "pos" : n < 0 ? "neg" : ""));
}

function buildTable(headers, rows, buildRow) {
  const thead = el("thead", {}, [
    el("tr", {}, headers.map(([label, cls]) => {
      const h = el("th", { text: label });
      if (cls) h.className = cls;
      return h;
    })),
  ]);
  const tbody = el("tbody", {}, rows.map(buildRow));
  const table = el("table", {}, [thead, tbody]);
  return el("div", { class: "table-wrap" }, [table]);
}

function setPanel(id, node, emptyMsg) {
  const panel = $(id);
  panel.replaceChildren();
  if (node) panel.appendChild(node);
  else panel.appendChild(el("p", { class: "muted", text: emptyMsg }));
}

// ── SVG charts ──────────────────────────────────────────────────────────────

const SVG_NS = "http://www.w3.org/2000/svg";

function svgEl(tag, attrs = {}) {
  const node = document.createElementNS(SVG_NS, tag);
  for (const [k, v] of Object.entries(attrs)) node.setAttribute(k, v);
  return node;
}

function niceRange(min, max) {
  if (min === max) { min -= 1; max += 1; }
  const pad = (max - min) * 0.12;
  return [min - pad, max + pad];
}

/* Linha de P&L acumulado, com marcador por trade e tooltip. */
function renderEquityChart(holder, points) {
  holder.replaceChildren();
  if (!points.length) {
    holder.appendChild(el("div", { class: "empty", text: "Sem trades ainda" }));
    return;
  }

  const W = 640, H = 240, m = { t: 14, r: 14, b: 24, l: 58 };
  const xs = points.map((p) => new Date(p.t).getTime());
  const ys = points.map((p) => Number(p.cum));
  const [yMin, yMax] = niceRange(Math.min(0, ...ys), Math.max(0, ...ys));
  const xMin = xs[0], xMax = xs[xs.length - 1] || xs[0] + 1;

  const X = (t) => xMax === xMin
    ? m.l + (W - m.l - m.r) / 2
    : m.l + ((t - xMin) / (xMax - xMin)) * (W - m.l - m.r);
  const Y = (v) => m.t + (1 - (v - yMin) / (yMax - yMin)) * (H - m.t - m.b);

  const svg = svgEl("svg", { viewBox: `0 0 ${W} ${H}`, preserveAspectRatio: "none" });

  // Grid horizontal + rótulos do eixo Y.
  for (const frac of [0, 0.5, 1]) {
    const v = yMin + frac * (yMax - yMin);
    const y = Y(v);
    svg.appendChild(svgEl("line", { x1: m.l, x2: W - m.r, y1: y, y2: y, stroke: "#1c2532", "stroke-width": 1 }));
    const label = svgEl("text", { x: m.l - 8, y: y + 4, fill: "#5a6a7e", "font-size": 11, "text-anchor": "end" });
    label.textContent = axisMoneyFmt.format(v);
    svg.appendChild(label);
  }

  // Linha do zero.
  if (yMin < 0 && yMax > 0) {
    const y0 = Y(0);
    svg.appendChild(svgEl("line", { x1: m.l, x2: W - m.r, y1: y0, y2: y0, stroke: "#3a4657", "stroke-width": 1, "stroke-dasharray": "4 4" }));
  }

  const last = ys[ys.length - 1];
  const color = last >= 0 ? "#3fb68b" : "#e5636e";

  // Área + linha em degraus (o P&L só muda quando um trade fecha).
  let d = `M ${X(xs[0])} ${Y(0)} L ${X(xs[0])} ${Y(ys[0])}`;
  for (let i = 1; i < xs.length; i++) d += ` L ${X(xs[i])} ${Y(ys[i - 1])} L ${X(xs[i])} ${Y(ys[i])}`;
  const areaD = d + ` L ${X(xs[xs.length - 1])} ${Y(0)} Z`;
  svg.appendChild(svgEl("path", { d: areaD, fill: color, opacity: 0.09 }));
  svg.appendChild(svgEl("path", { d, fill: "none", stroke: color, "stroke-width": 2, "stroke-linejoin": "round" }));

  // Marcadores.
  points.forEach((p, i) => {
    svg.appendChild(svgEl("circle", { cx: X(xs[i]), cy: Y(ys[i]), r: 3, fill: color }));
  });

  // Rótulos do eixo X (primeira e última data).
  const x0 = svgEl("text", { x: m.l, y: H - 6, fill: "#5a6a7e", "font-size": 11 });
  x0.textContent = etDate.format(new Date(xs[0]));
  svg.appendChild(x0);
  const x1 = svgEl("text", { x: W - m.r, y: H - 6, fill: "#5a6a7e", "font-size": 11, "text-anchor": "end" });
  x1.textContent = etDate.format(new Date(xs[xs.length - 1]));
  svg.appendChild(x1);

  const tip = el("div", { class: "chart-tip" });
  holder.appendChild(svg);
  holder.appendChild(tip);

  svg.addEventListener("mousemove", (ev) => {
    const rect = svg.getBoundingClientRect();
    const mx = ((ev.clientX - rect.left) / rect.width) * W;
    let best = 0, bestDist = Infinity;
    xs.forEach((t, i) => {
      const dist = Math.abs(X(t) - mx);
      if (dist < bestDist) { bestDist = dist; best = i; }
    });
    const p = points[best];
    tip.style.display = "block";
    tip.style.left = Math.min(ev.clientX - rect.left + 12, rect.width - 170) + "px";
    tip.style.top = (ev.clientY - rect.top - 10) + "px";
    tip.textContent = `${fmtET(p.t)} · ${p.symbol} ${fmtMoney(p.pnl)} → ${fmtMoney(p.cum)}`;
  });
  svg.addEventListener("mouseleave", () => { tip.style.display = "none"; });
}

/* Barras de P&L por dia. */
function renderDailyChart(holder, days) {
  holder.replaceChildren();
  if (!days.length) {
    holder.appendChild(el("div", { class: "empty", text: "Sem trades ainda" }));
    return;
  }

  const data = [...days].reverse(); // API devolve mais recente primeiro
  const W = 480, H = 240, m = { t: 14, r: 10, b: 24, l: 58 };
  const vals = data.map((d) => Number(d.net_pnl));
  const [yMin, yMax] = niceRange(Math.min(0, ...vals), Math.max(0, ...vals));
  const Y = (v) => m.t + (1 - (v - yMin) / (yMax - yMin)) * (H - m.t - m.b);

  const svg = svgEl("svg", { viewBox: `0 0 ${W} ${H}`, preserveAspectRatio: "none" });

  for (const frac of [0, 0.5, 1]) {
    const v = yMin + frac * (yMax - yMin);
    const y = Y(v);
    svg.appendChild(svgEl("line", { x1: m.l, x2: W - m.r, y1: y, y2: y, stroke: "#1c2532" }));
    const label = svgEl("text", { x: m.l - 8, y: y + 4, fill: "#5a6a7e", "font-size": 11, "text-anchor": "end" });
    label.textContent = axisMoneyFmt.format(v);
    svg.appendChild(label);
  }

  const y0 = Y(0);
  svg.appendChild(svgEl("line", { x1: m.l, x2: W - m.r, y1: y0, y2: y0, stroke: "#3a4657" }));

  const span = W - m.l - m.r;
  const step = span / data.length;
  const barW = Math.max(3, Math.min(26, step * 0.65));

  const tip = el("div", { class: "chart-tip" });

  data.forEach((d, i) => {
    const v = Number(d.net_pnl);
    const x = m.l + i * step + (step - barW) / 2;
    const y = Y(Math.max(0, v));
    const h = Math.max(1.5, Math.abs(Y(v) - y0));
    const bar = svgEl("rect", {
      x, y: v >= 0 ? y : y0, width: barW, height: h, rx: 1.5,
      fill: v >= 0 ? "#3fb68b" : "#e5636e", opacity: 0.85,
    });
    bar.addEventListener("mousemove", (ev) => {
      const rect = svg.getBoundingClientRect();
      tip.style.display = "block";
      tip.style.left = Math.min(ev.clientX - rect.left + 12, rect.width - 160) + "px";
      tip.style.top = (ev.clientY - rect.top - 10) + "px";
      tip.textContent = `${fmtDay(d.day)} · ${fmtMoney(v)} · ${d.trades} trade${d.trades === 1 ? "" : "s"}`;
    });
    bar.addEventListener("mouseleave", () => { tip.style.display = "none"; });
    svg.appendChild(bar);
  });

  const xa = svgEl("text", { x: m.l, y: H - 6, fill: "#5a6a7e", "font-size": 11 });
  xa.textContent = fmtDay(data[0].day);
  svg.appendChild(xa);
  const xb = svgEl("text", { x: W - m.r, y: H - 6, fill: "#5a6a7e", "font-size": 11, "text-anchor": "end" });
  xb.textContent = fmtDay(data[data.length - 1].day);
  svg.appendChild(xb);

  holder.appendChild(svg);
  holder.appendChild(tip);
}

/* Sparkline de fechamento nos cards de instância. */
function renderSparkline(holder, candles) {
  holder.replaceChildren();
  if (candles.length < 2) return;

  const W = 240, H = 36;
  const ys = candles.map((c) => Number(c.close));
  const min = Math.min(...ys), max = Math.max(...ys);
  const Y = (v) => max === min ? H / 2 : 3 + (1 - (v - min) / (max - min)) * (H - 6);
  const X = (i) => (i / (ys.length - 1)) * W;

  const up = ys[ys.length - 1] >= ys[0];
  const color = up ? "#3fb68b" : "#e5636e";
  const d = ys.map((v, i) => `${i ? "L" : "M"} ${X(i).toFixed(1)} ${Y(v).toFixed(1)}`).join(" ");

  const svg = svgEl("svg", { viewBox: `0 0 ${W} ${H}`, preserveAspectRatio: "none" });
  svg.appendChild(svgEl("path", {
    d: `${d} L ${W} ${H} L 0 ${H} Z`, fill: color, opacity: 0.08,
  }));
  svg.appendChild(svgEl("path", { d, fill: "none", stroke: color, "stroke-width": 1.5 }));
  holder.appendChild(svg);
}

// ── Fetch ───────────────────────────────────────────────────────────────────

async function getJSON(url) {
  const res = await fetch(url, { cache: "no-store" });
  if (!res.ok) throw new Error(`${url} → HTTP ${res.status}`);
  return res.json();
}

// ── Renderização das seções ─────────────────────────────────────────────────

function renderOverview(o) {
  $("health-dot").className = "brand-dot ok";

  const phasePill = $("pill-phase");
  const phases = {
    open: ["Pregão aberto", "ok"],
    pre_window: ["Antes da janela (abre 09:25 ET)", "warn"],
    after_window: ["Pregão encerrado", ""],
    weekend: ["Fim de semana", ""],
  };
  const [phaseLabel, phaseCls] = phases[o.market_phase] || [o.market_phase, ""];
  phasePill.textContent = phaseLabel;
  phasePill.className = "pill " + phaseCls;

  const gw = $("pill-gateway");
  gw.textContent = o.gateway_port_open ? "Gateway no ar" : "Gateway fora do ar";
  gw.className = "pill " + (o.gateway_port_open ? "ok" : (o.market_phase === "open" ? "bad" : "warn"));

  $("pill-clock").textContent = o.now_et.slice(11, 16) + " ET";

  const pnlToday = Number(o.today.net_pnl);
  const kToday = $("kpi-pnl-today");
  kToday.textContent = fmtMoney(o.today.net_pnl);
  kToday.className = "kpi-value " + (pnlToday > 0 ? "pos" : pnlToday < 0 ? "neg" : "");
  $("kpi-trades-today").textContent = `${o.today.trades} trade${o.today.trades === 1 ? "" : "s"} hoje`;

  const pnlTotal = Number(o.total.net_pnl);
  const kTotal = $("kpi-pnl-total");
  kTotal.textContent = fmtMoney(o.total.net_pnl);
  kTotal.className = "kpi-value " + (pnlTotal > 0 ? "pos" : pnlTotal < 0 ? "neg" : "");
  $("kpi-trades-total").textContent = `${o.total.trades} trades no total`;

  const winrate = o.total.trades > 0 ? (100 * o.total.wins) / o.total.trades : null;
  $("kpi-winrate").textContent = winrate === null ? "—" : winrate.toFixed(0) + "%";
  $("kpi-r-total").textContent = `${fmtR(o.total.sum_r)} acumulado`;

  $("kpi-signals-today").textContent =
    `${o.signals_today.accepted} / ${o.signals_today.rejected}`;

  $("kpi-open-orders").textContent = String(o.open_orders.today);
  // Toda ordem do bot é TIF day: status "aberto" de dias anteriores é ordem
  // que nunca foi reconciliada no banco, não ordem viva no broker.
  $("kpi-orders-sub").textContent = o.open_orders.stale > 0
    ? `⚠ ${o.open_orders.stale} antiga${o.open_orders.stale === 1 ? "" : "s"} sem reconciliar`
    : `último candle ${fmtAge(o.last_candle_at)}`;

  $("footer-db").textContent = `Banco: ${o.db_target}`;

  const banner = $("banner-error");
  if (o.last_error && (Date.now() - new Date(o.last_error.timestamp).getTime()) < 24 * 3600 * 1000) {
    banner.textContent =
      `⚠ ${o.last_error.level.toUpperCase()} · ${o.last_error.component}/${o.last_error.event_type}` +
      ` · ${fmtET(o.last_error.timestamp)} ET — ${o.last_error.message}`;
    banner.classList.remove("hidden");
  } else {
    banner.classList.add("hidden");
  }

  return o;
}

function instanceDot(inst, overview) {
  if (!overview || overview.market_phase !== "open") return "idle";
  if (!inst.last_candle_at) return "warn";
  const ageMin = (Date.now() - new Date(inst.last_candle_at).getTime()) / 60000;
  return ageMin <= 30 ? "ok" : "warn";
}

function renderInstances(instances, overview, sparks) {
  const holder = $("instances");
  holder.replaceChildren();

  instances.forEach((inst) => {
    const dot = instanceDot(inst, overview);
    const spark = el("div", { class: "inst-spark" });
    const candles = sparks.get(inst.symbol);
    if (candles) renderSparkline(spark, candles);

    const lastSignal = inst.last_signal_at
      ? `${fmtAge(inst.last_signal_at)}${inst.last_signal_status ? " (" + inst.last_signal_status + ")" : ""}`
      : "nunca";
    const lastTrade = inst.last_trade_at
      ? `${fmtAge(inst.last_trade_at)} (${fmtMoney(inst.last_trade_pnl)})`
      : "nunca";

    holder.appendChild(el("div", { class: "inst" }, [
      el("div", { class: "inst-head" }, [
        el("span", { class: `inst-dot ${dot}`, title: dot === "ok" ? "Dados chegando" : dot === "warn" ? "Sem dados recentes" : "Fora da janela de pregão" }),
        el("span", { class: "inst-symbol", text: inst.symbol }),
        el("span", { class: "inst-name", text: `#${inst.client_id} ${inst.name}` }),
      ]),
      el("div", { class: "inst-strategy", text: inst.strategy }),
      spark,
      el("div", { class: "inst-meta" }, [
        el("span", {}, [el("b", { text: "candle " }), document.createTextNode(fmtAge(inst.last_candle_at))]),
        el("span", {}, [el("b", { text: "sinal " }), document.createTextNode(lastSignal)]),
      ]),
      el("div", { class: "inst-meta" }, [
        el("span", {}, [el("b", { text: "último trade " }), document.createTextNode(lastTrade)]),
      ]),
    ]));
  });

  $("instances-note").textContent = `${instances.length} instâncias configuradas`;
}

function renderTrades(rows) {
  if (!rows.length) return setPanel("panel-trades", null, "Nenhum trade registrado ainda.");
  setPanel("panel-trades", buildTable(
    [["Saída (ET)"], ["Símbolo"], ["Estratégia"], ["Dir."], ["Entrada", "num"], ["Saída", "num"], ["Qtd", "num"], ["P&L", "num"], ["R", "num"], ["Motivo"]],
    rows,
    (t) => el("tr", {}, [
      td(fmtET(t.exit_time)),
      td(t.symbol, "sym"),
      td(t.strategy_id),
      tdChip(t.direction, t.direction),
      td(fmtNum(t.entry_price), "num"),
      td(fmtNum(t.exit_price), "num"),
      td(fmtNum(t.quantity, 0), "num"),
      pnlTd(t.net_pnl),
      td(fmtR(t.result_in_r), "num " + (Number(t.result_in_r) > 0 ? "pos" : "neg")),
      tdChip(t.exit_reason, t.exit_reason),
    ])
  ));
}

function renderSignals(rows) {
  if (!rows.length) return setPanel("panel-signals", null, "Nenhum sinal registrado ainda.");
  setPanel("panel-signals", buildTable(
    [["Quando (ET)"], ["Símbolo"], ["Estratégia"], ["TF"], ["Dir."], ["Status"], ["Entrada", "num"], ["Stop", "num"], ["Alvo", "num"], ["Motivo"]],
    rows,
    (s) => el("tr", {}, [
      td(fmtET(s.timestamp)),
      td(s.symbol, "sym"),
      td(s.strategy_id),
      td(s.timeframe),
      tdChip(s.direction, s.direction || "muted"),
      tdChip(s.status, s.status),
      td(s.entry_price ? fmtNum(s.entry_price) : "—", "num"),
      td(s.stop_price ? fmtNum(s.stop_price) : "—", "num"),
      td(s.target_price ? fmtNum(s.target_price) : "—", "num"),
      td(s.rejection_reason || s.entry_reason || "—", "wrap muted"),
    ])
  ));
}

function renderOrders(data) {
  const panel = $("panel-orders");
  panel.replaceChildren();

  if (!data.orders.length && !data.fills.length) {
    panel.appendChild(el("p", { class: "muted", text: "Nenhuma ordem registrada ainda." }));
    return;
  }

  if (data.orders.length) {
    panel.appendChild(el("h3", { text: "Ordens", class: "muted" }));
    panel.appendChild(buildTable(
      [["Criada (ET)"], ["Símbolo"], ["Lado"], ["Tipo"], ["Status"], ["Qtd", "num"], ["Executada", "num"], ["Preço", "num"], ["Stop", "num"], ["Preço médio", "num"], ["Broker"]],
      data.orders,
      (o) => el("tr", {}, [
        td(fmtET(o.created_at)),
        td(o.symbol, "sym"),
        tdChip(o.side, o.side === "buy" ? "long" : "short"),
        td(o.order_type),
        tdChip(o.status, o.status),
        td(fmtNum(o.quantity, 0), "num"),
        td(fmtNum(o.filled_quantity, 0), "num"),
        td(o.price ? fmtNum(o.price) : "—", "num"),
        td(o.stop_price ? fmtNum(o.stop_price) : "—", "num"),
        td(o.avg_fill_price ? fmtNum(o.avg_fill_price) : "—", "num"),
        td(o.broker),
      ])
    ));
  }

  if (data.fills.length) {
    panel.appendChild(el("h3", { text: "Fills", class: "muted" }));
    panel.appendChild(buildTable(
      [["Quando (ET)"], ["Símbolo"], ["Lado"], ["Preço", "num"], ["Qtd", "num"], ["Comissão", "num"]],
      data.fills,
      (f) => el("tr", {}, [
        td(fmtET(f.timestamp)),
        td(f.symbol, "sym"),
        tdChip(f.side, f.side === "buy" ? "long" : "short"),
        td(fmtNum(f.fill_price), "num"),
        td(fmtNum(f.quantity, 0), "num"),
        td(fmtNum(f.commission), "num"),
      ])
    ));
  }
}

function renderStrategies(rows) {
  if (!rows.length) return setPanel("panel-strategies", null, "Nenhuma estratégia com atividade ainda.");
  setPanel("panel-strategies", buildTable(
    [["Estratégia"], ["Trades", "num"], ["Win rate", "num"], ["P&L", "num"], ["R acum.", "num"], ["Sinais aceitos", "num"], ["Rejeitados", "num"], ["Último sinal"], ["Último trade"]],
    rows,
    (s) => {
      const wr = s.trades > 0 ? ((100 * s.wins) / s.trades).toFixed(0) + "%" : "—";
      return el("tr", {}, [
        td(s.strategy_id, "sym"),
        td(String(s.trades), "num"),
        td(wr, "num"),
        pnlTd(s.net_pnl),
        td(fmtR(s.sum_r), "num " + (Number(s.sum_r) > 0 ? "pos" : Number(s.sum_r) < 0 ? "neg" : "")),
        td(String(s.accepted_signals), "num"),
        td(String(s.rejected_signals), "num"),
        td(fmtAge(s.last_signal_at)),
        td(fmtAge(s.last_trade_at)),
      ]);
    }
  ));
}

function renderEvents(rows) {
  if (!rows.length) return setPanel("panel-events", null, "Nenhum evento registrado.");
  setPanel("panel-events", buildTable(
    [["Quando (ET)"], ["Nível"], ["Componente"], ["Tipo"], ["Mensagem"]],
    rows,
    (e) => el("tr", {}, [
      td(fmtET(e.timestamp)),
      tdChip(e.level, e.level),
      td(e.component),
      td(e.event_type),
      td(e.message, "wrap"),
    ])
  ));
}

function renderBacktests(rows) {
  if (!rows.length) return setPanel("panel-backtests", null, "Nenhum backtest registrado.");
  setPanel("panel-backtests", buildTable(
    [["Rodado (ET)"], ["Símbolo"], ["Estratégia"], ["TF"], ["Período"], ["Capital", "num"], ["Equity final", "num"], ["Retorno", "num"], ["Label"]],
    rows,
    (b) => {
      const ret = (Number(b.final_equity) / Number(b.initial_capital) - 1) * 100;
      return el("tr", {}, [
        td(fmtET(b.created_at)),
        td(b.symbol, "sym"),
        td(b.strategy_id),
        td(b.timeframe),
        td(`${etDate.format(new Date(b.period_start))} → ${etDate.format(new Date(b.period_end))}`),
        td(moneyFmt.format(Number(b.initial_capital)), "num"),
        td(moneyFmt.format(Number(b.final_equity)), "num"),
        td((ret > 0 ? "+" : "") + ret.toFixed(2) + "%", "num " + (ret > 0 ? "pos" : ret < 0 ? "neg" : "")),
        td(b.label || "—", "muted"),
      ]);
    }
  ));
}

// ── Ciclo de atualização ────────────────────────────────────────────────────

let refreshing = false;

async function refresh() {
  if (refreshing) return;
  refreshing = true;
  $("refresh-btn").classList.add("spinning");

  try {
    const overview = await getJSON("/api/overview").then(renderOverview);

    const [instances, equity, daily, trades, signals, orders, strategies, events, backtests] =
      await Promise.all([
        getJSON("/api/instances"),
        getJSON("/api/equity-curve"),
        getJSON("/api/pnl-daily?days=90"),
        getJSON("/api/trades?limit=100"),
        getJSON("/api/signals?limit=100"),
        getJSON("/api/orders?limit=50"),
        getJSON("/api/strategies"),
        getJSON("/api/events?limit=100"),
        getJSON("/api/backtests?limit=30"),
      ]);

    // Sparklines: um fetch por símbolo distinto.
    const symbols = [...new Set(instances.map((i) => i.symbol))];
    const sparkPairs = await Promise.all(symbols.map(async (s) => {
      try {
        return [s, await getJSON(`/api/candles?symbol=${encodeURIComponent(s)}&timeframe=15m&limit=54`)];
      } catch { return [s, null]; }
    }));
    const sparks = new Map(sparkPairs.filter(([, v]) => v));

    renderInstances(instances, overview, sparks);
    renderEquityChart($("chart-equity"), equity);
    renderDailyChart($("chart-daily"), daily);
    renderTrades(trades);
    renderSignals(signals);
    renderOrders(orders);
    renderStrategies(strategies);
    renderEvents(events);
    renderBacktests(backtests);

    $("footer-updated").textContent =
      "Atualizado às " + new Date().toLocaleTimeString("pt-BR");
  } catch (err) {
    $("health-dot").className = "brand-dot bad";
    const banner = $("banner-error");
    banner.textContent = "Falha ao atualizar o painel: " + err.message;
    banner.classList.remove("hidden");
  } finally {
    refreshing = false;
    $("refresh-btn").classList.remove("spinning");
  }
}

// Tabs.
$("tabs").addEventListener("click", (ev) => {
  const btn = ev.target.closest(".tab");
  if (!btn) return;
  document.querySelectorAll(".tab").forEach((t) => t.classList.toggle("active", t === btn));
  document.querySelectorAll(".tab-panel").forEach((p) =>
    p.classList.toggle("hidden", p.id !== "panel-" + btn.dataset.tab));
});

$("refresh-btn").addEventListener("click", refresh);

document.addEventListener("visibilitychange", () => {
  if (!document.hidden) refresh();
});

refresh();
setInterval(() => { if (!document.hidden) refresh(); }, REFRESH_MS);
