//! Reporte HTML standalone (offline): CSS y JS embebidos, sin recursos externos.

use metrics::{LabelStats, RunSummary, SampleKind};

fn esc(s: &str) -> String {
    html_escape::encode_text(s).into_owned()
}
fn f1(v: f64) -> String {
    format!("{v:.1}")
}
fn pct(v: f64) -> String {
    format!("{:.2}%", v * 100.0)
}

const CSS: &str = r#"
:root{--bg:#f8fafc;--card:#fff;--ink:#0f172a;--muted:#64748b;--line:#e2e8f0;--accent:#2563eb;--bad:#dc2626;--good:#16a34a}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--ink);font:14px/1.5 system-ui,-apple-system,Segoe UI,Roboto,sans-serif}
header{background:linear-gradient(120deg,#0f172a,#1e3a8a);color:#fff;padding:24px 32px}
header h1{margin:0 0 4px;font-size:20px}header .sub{opacity:.8;font-size:13px}
.wrap{max-width:1100px;margin:0 auto;padding:24px 32px}
.cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:12px;margin:0 0 24px}
.card{background:var(--card);border:1px solid var(--line);border-radius:10px;padding:14px 16px}
.card .k{color:var(--muted);font-size:12px;text-transform:uppercase;letter-spacing:.04em}
.card .v{font-size:22px;font-weight:600;margin-top:4px}
.card .v.bad{color:var(--bad)}.card .v.good{color:var(--good)}
h2{font-size:15px;margin:28px 0 12px}
.charts{display:grid;grid-template-columns:repeat(auto-fit,minmax(300px,1fr));gap:16px}
.chart{background:var(--card);border:1px solid var(--line);border-radius:10px;padding:12px}
.chart h3{margin:0 0 8px;font-size:13px;color:var(--muted);font-weight:600}
canvas{width:100%;height:180px}
table{width:100%;border-collapse:collapse;background:var(--card);border:1px solid var(--line);border-radius:10px;overflow:hidden}
th,td{padding:9px 12px;text-align:right;border-bottom:1px solid var(--line);font-variant-numeric:tabular-nums}
th:first-child,td:first-child{text-align:left}
th{background:#f1f5f9;color:var(--muted);font-size:12px;text-transform:uppercase;letter-spacing:.03em}
tr:last-child td{border-bottom:none}
.tag{display:inline-block;font-size:11px;padding:1px 7px;border-radius:999px;background:#eef2ff;color:#3730a3}
.err{color:var(--bad);font-weight:600}.ok{color:var(--good)}
footer{color:var(--muted);font-size:12px;padding:16px 32px;text-align:center}
"#;

const JS: &str = r#"
function lineChart(id,pts,color,yfmt){
  const c=document.getElementById(id); if(!c) return;
  const r=window.devicePixelRatio||1; c.width=c.clientWidth*r; c.height=c.clientHeight*r;
  const ctx=c.getContext('2d'); ctx.scale(r,r);
  const W=c.clientWidth,H=c.clientHeight,pad=34; ctx.clearRect(0,0,W,H);
  if(!pts.length){ctx.fillStyle='#94a3b8';ctx.font='12px system-ui';ctx.fillText('sin datos',pad,H/2);return;}
  const xs=pts.map(p=>p.x),ys=pts.map(p=>p.y);
  const xmin=Math.min(...xs),xmax=Math.max(...xs,1),ymax=Math.max(...ys,1)*1.15;
  const X=x=>pad+(W-2*pad)*((x-xmin)/((xmax-xmin)||1));
  const Y=y=>H-pad-(H-2*pad)*(y/ymax);
  ctx.strokeStyle='#e2e8f0';ctx.lineWidth=1;ctx.beginPath();
  ctx.moveTo(pad,H-pad);ctx.lineTo(W-pad,H-pad);ctx.moveTo(pad,pad);ctx.lineTo(pad,H-pad);ctx.stroke();
  ctx.fillStyle=color+'22';ctx.beginPath();ctx.moveTo(X(pts[0].x),H-pad);
  pts.forEach(p=>ctx.lineTo(X(p.x),Y(p.y)));ctx.lineTo(X(pts[pts.length-1].x),H-pad);ctx.closePath();ctx.fill();
  ctx.strokeStyle=color;ctx.lineWidth=2;ctx.beginPath();
  pts.forEach((p,i)=>{const xx=X(p.x),yy=Y(p.y);i?ctx.lineTo(xx,yy):ctx.moveTo(xx,yy);});ctx.stroke();
  ctx.fillStyle='#64748b';ctx.font='11px system-ui';
  ctx.fillText(yfmt(ymax),4,pad+2);ctx.fillText('0',4,H-pad);
  ctx.fillText(xmin+'s',pad,H-pad+13);ctx.fillText(xmax+'s',W-pad-16,H-pad+13);
}
const ts=DATA.timeseries||[];
function draw(){
 lineChart('c_tp',ts.map(p=>({x:p.t_secs,y:p.throughput})),'#2563eb',v=>v.toFixed(0)+'/s');
 lineChart('c_p95',ts.map(p=>({x:p.t_secs,y:p.p95_ms})),'#7c3aed',v=>v.toFixed(0)+'ms');
 lineChart('c_err',ts.map(p=>({x:p.t_secs,y:p.error_rate*100})),'#dc2626',v=>v.toFixed(1)+'%');
 lineChart('c_vu',ts.map(p=>({x:p.t_secs,y:p.active_vus})),'#0891b2',v=>v.toFixed(0));
}
draw();window.addEventListener('resize',draw);
"#;

fn label_row(l: &LabelStats) -> String {
    let kind = match l.kind {
        SampleKind::Transaction => "<span class=\"tag\">tx</span> ",
        SampleKind::Kafka => "<span class=\"tag\">kafka</span> ",
        SampleKind::Http => "",
    };
    let errcell = if l.errors > 0 {
        format!(
            "<td class=\"err\">{} ({})</td>",
            l.errors,
            pct(l.error_rate)
        )
    } else {
        "<td class=\"ok\">0</td>".to_string()
    };
    format!(
        "<tr><td>{kind}{name}</td><td>{count}</td>{errcell}<td>{tp}</td><td>{p50}</td><td>{p95}</td><td>{p99}</td><td>{max}</td></tr>",
        name = esc(&l.label),
        count = l.count,
        tp = f1(l.throughput_per_sec),
        p50 = f1(l.p50_ms),
        p95 = f1(l.p95_ms),
        p99 = f1(l.p99_ms),
        max = f1(l.max_ms),
    )
}

/// Genera el reporte HTML completo (offline).
pub fn html_report(s: &RunSummary) -> String {
    let data = serde_json::to_string(s)
        .unwrap_or_else(|_| "{}".into())
        .replace("</", "<\\/");
    let o = &s.overall;

    let err_class = if o.error_rate > 0.0 { "bad" } else { "good" };
    let cards = format!(
        r#"<div class="cards">
<div class="card"><div class="k">Requests</div><div class="v">{reqs}</div></div>
<div class="card"><div class="k">Error rate</div><div class="v {ec}">{er}</div></div>
<div class="card"><div class="k">Throughput</div><div class="v">{tp}/s</div></div>
<div class="card"><div class="k">p50</div><div class="v">{p50} ms</div></div>
<div class="card"><div class="k">p90</div><div class="v">{p90} ms</div></div>
<div class="card"><div class="k">p95</div><div class="v">{p95} ms</div></div>
<div class="card"><div class="k">p99</div><div class="v">{p99} ms</div></div>
<div class="card"><div class="k">p99.9</div><div class="v">{p999} ms</div></div>
</div>"#,
        reqs = o.count,
        ec = err_class,
        er = pct(o.error_rate),
        tp = f1(o.throughput_per_sec),
        p50 = f1(o.p50_ms),
        p90 = f1(o.p90_ms),
        p95 = f1(o.p95_ms),
        p99 = f1(o.p99_ms),
        p999 = f1(o.p999_ms),
    );

    let rows: String = s.labels.iter().map(label_row).collect();

    let errors_section = if s.errors.is_empty() {
        String::new()
    } else {
        let er: String = s
            .errors
            .iter()
            .map(|e| format!("<tr><td>{}</td><td>{}</td></tr>", esc(&e.message), e.count))
            .collect();
        format!(
            "<h2>Errores</h2><table><thead><tr><th>Mensaje</th><th>Conteo</th></tr></thead><tbody>{er}</tbody></table>"
        )
    };

    format!(
        r#"<!doctype html><html lang="es"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>perfkit — {title}</title><style>{css}</style></head><body>
<header><h1>{title}</h1><div class="sub">run {run} · inicio {started} · duración {dur:.1}s · {vus} VUs · {tg} thread group(s)</div></header>
<div class="wrap">
{cards}
<h2>Series temporales</h2>
<div class="charts">
<div class="chart"><h3>Throughput (req/s)</h3><canvas id="c_tp"></canvas></div>
<div class="chart"><h3>Latencia p95 (ms)</h3><canvas id="c_p95"></canvas></div>
<div class="chart"><h3>Error rate (%)</h3><canvas id="c_err"></canvas></div>
<div class="chart"><h3>VUs activos</h3><canvas id="c_vu"></canvas></div>
</div>
<h2>Por sampler / transacción (ordenado por p95)</h2>
<table><thead><tr><th>Etiqueta</th><th># </th><th>Errores</th><th>Thr/s</th><th>p50</th><th>p95</th><th>p99</th><th>max</th></tr></thead>
<tbody>{rows}</tbody></table>
{errors_section}
</div>
<footer>Generado por perfkit · reporte offline autocontenido</footer>
<script>const DATA={data};</script><script>{js}</script>
</body></html>"#,
        title = esc(&s.scenario_name),
        css = CSS,
        run = esc(&s.run_id),
        started = esc(&s.started_at),
        dur = s.duration_secs,
        vus = s.config.virtual_users,
        tg = s.config.thread_groups,
        cards = cards,
        rows = rows,
        errors_section = errors_section,
        data = data,
        js = JS,
    )
}
