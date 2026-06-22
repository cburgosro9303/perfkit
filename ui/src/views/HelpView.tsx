import React, { useEffect, useState } from "react";
import { Badge, Card } from "../components/ui";
import type { BadgeColor } from "../components/ui";

// ─── Tabla de contenidos ──────────────────────────────────────────────────────

interface TocEntry {
  id: string;
  label: string;
}

const TOC: TocEntry[] = [
  { id: "intro", label: "Introducción" },
  { id: "crear-importar", label: "Crear o importar un plan" },
  { id: "estructura", label: "Estructura del plan" },
  { id: "controladores", label: "Tipos de controladores" },
  { id: "assertions", label: "Assertions, extractores y variables" },
  { id: "probar", label: "Probar petición e inspección" },
  { id: "ejecutar", label: "Ejecutar" },
  { id: "reporte", label: "Reporte" },
  { id: "import-export", label: "Importar/Exportar y migración" },
  { id: "historico", label: "Histórico y SLA en CI" },
  { id: "avanzado", label: "Avanzado" },
  { id: "casos", label: "Casos de uso" },
];

// ─── Primitivas de contenido ───────────────────────────────────────────────────

const Section: React.FC<{ id: string; title: string; children: React.ReactNode }> = ({
  id,
  title,
  children,
}) => (
  <section id={id} className="scroll-mt-6">
    <h2 className="text-xl font-bold text-slate-900 tracking-tight">{title}</h2>
    <div className="mt-3 flex flex-col gap-3 text-sm leading-relaxed text-slate-600">
      {children}
    </div>
  </section>
);

const SubHeading: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <h3 className="text-sm font-semibold text-slate-800 mt-2">{children}</h3>
);

const Code: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <code className="px-1.5 py-0.5 rounded bg-slate-100 text-[0.85em] font-mono text-indigo-700 border border-slate-200">
    {children}
  </code>
);

const CodeBlock: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <pre className="rounded-lg bg-slate-900 text-slate-100 text-xs font-mono p-3.5 overflow-x-auto leading-relaxed">
    <code>{children}</code>
  </pre>
);

interface Col {
  key: string;
  header: string;
}

const Table: React.FC<{ cols: Col[]; rows: Record<string, React.ReactNode>[] }> = ({
  cols,
  rows,
}) => (
  <div className="overflow-x-auto rounded-lg border border-slate-200">
    <table className="w-full text-sm">
      <thead>
        <tr className="bg-slate-50 text-left text-xs font-semibold text-slate-500 uppercase tracking-wide">
          {cols.map((c) => (
            <th key={c.key} className="px-3 py-2.5 align-bottom">
              {c.header}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map((r, i) => (
          <tr
            key={i}
            className={`border-t border-slate-100 align-top ${i % 2 ? "bg-slate-50/40" : "bg-white"}`}
          >
            {cols.map((c) => (
              <td key={c.key} className="px-3 py-2.5 text-slate-600">
                {r[c.key]}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  </div>
);

const DefList: React.FC<{ items: { term: React.ReactNode; def: React.ReactNode }[] }> = ({
  items,
}) => (
  <dl className="flex flex-col gap-2">
    {items.map((it, i) => (
      <div key={i} className="flex flex-col sm:flex-row sm:gap-3">
        <dt className="sm:w-40 shrink-0 font-semibold text-slate-700">{it.term}</dt>
        <dd className="text-slate-600">{it.def}</dd>
      </div>
    ))}
  </dl>
);

const FidelityBadge: React.FC<{ status: "migrated" | "assisted" | "unsupported" | "ignored" }> = ({
  status,
}) => {
  const map: Record<string, { color: BadgeColor; label: string }> = {
    migrated: { color: "green", label: "migrated" },
    assisted: { color: "amber", label: "assisted" },
    unsupported: { color: "red", label: "unsupported" },
    ignored: { color: "slate", label: "ignored" },
  };
  const { color, label } = map[status];
  return <Badge color={color}>{label}</Badge>;
};

// ─── Vista ──────────────────────────────────────────────────────────────────

export const HelpView: React.FC = () => {
  const [active, setActive] = useState<string>(TOC[0].id);

  // Resalta en la TOC la sección visible (scroll spy con IntersectionObserver).
  useEffect(() => {
    const els = TOC.map((t) => document.getElementById(t.id)).filter(
      (e): e is HTMLElement => e !== null,
    );
    if (els.length === 0) return;
    const observer = new IntersectionObserver(
      (entries) => {
        const visible = entries
          .filter((e) => e.isIntersecting)
          .sort((a, b) => a.boundingClientRect.top - b.boundingClientRect.top);
        if (visible[0]) setActive(visible[0].target.id);
      },
      { rootMargin: "0px 0px -70% 0px", threshold: 0 },
    );
    els.forEach((el) => observer.observe(el));
    return () => observer.disconnect();
  }, []);

  return (
    <div className="h-full overflow-y-auto">
      <div className="mx-auto max-w-5xl px-6 py-8 flex gap-8">
        {/* TOC pegajosa */}
        <aside className="hidden lg:block w-56 shrink-0">
          <div className="sticky top-8">
            <p className="text-xs font-semibold text-slate-400 uppercase tracking-wide mb-3 px-3">
              Guía de perfkit
            </p>
            <nav className="flex flex-col gap-0.5">
              {TOC.map((t) => (
                <a
                  key={t.id}
                  href={`#${t.id}`}
                  className={`px-3 py-1.5 rounded-md text-sm transition-colors ${
                    active === t.id
                      ? "bg-indigo-50 text-indigo-700 font-medium"
                      : "text-slate-500 hover:text-slate-800 hover:bg-slate-100"
                  }`}
                >
                  {t.label}
                </a>
              ))}
            </nav>
          </div>
        </aside>

        {/* Contenido */}
        <article className="flex-1 min-w-0 flex flex-col gap-10 pb-20">
          {/* Encabezado */}
          <header>
            <h1 className="text-2xl font-bold text-slate-900 tracking-tight">Ayuda de perfkit</h1>
            <p className="mt-1 text-sm text-slate-500">
              Guía completa de perfkit Studio: del plan al reporte, importación desde JMeter y
              control de regresiones en CI.
            </p>
          </header>

          {/* 1. Introducción */}
          <Section id="intro" title="Introducción">
            <p>
              <strong className="text-slate-800">perfkit</strong> es una suite de pruebas de
              rendimiento (sucesora de Apache JMeter para QA tradicional). En lugar de XML, el plan
              vive en un <strong>IR canónico</strong> (representación intermedia, YAML legible y
              versionable): el motor ejecuta ese IR, no el JMX. Importador, motor, reportes, CLI y
              esta UI se comunican a través del mismo IR.
            </p>
            <p>El flujo de trabajo típico en Studio es:</p>
            <div className="flex flex-wrap items-center gap-2 text-xs font-medium">
              {[
                "Inicio",
                "Crear o importar",
                "Editar el plan",
                "Probar petición",
                "Ejecutar",
                "Reporte",
                "Exportar / Histórico",
              ].map((step, i, arr) => (
                <React.Fragment key={step}>
                  <span className="px-2.5 py-1 rounded-md bg-indigo-50 text-indigo-700 border border-indigo-100">
                    {step}
                  </span>
                  {i < arr.length - 1 && <span className="text-slate-300">→</span>}
                </React.Fragment>
              ))}
            </div>
            <p>
              La barra lateral izquierda te lleva por estos pasos. La etiqueta inferior indica el
              modo: <Badge color="green">Nativa</Badge> (app de escritorio, motor real in-process)
              o <Badge color="slate">Demo</Badge> (navegador, con datos de muestra para explorar la
              interfaz).
            </p>
          </Section>

          {/* 2. Crear o importar */}
          <Section id="crear-importar" title="Crear o importar un plan">
            <SubHeading>Crear desde cero</SubHeading>
            <p>
              Pulsa <Code>＋ Nuevo plan</Code> (arriba a la derecha, siempre disponible) para
              empezar con un escenario vacío con la estructura nativa de perfkit: un grupo de hilos
              y un paso inicial que puedes editar de inmediato en la vista <strong>Plan</strong>.
            </p>
            <SubHeading>Importar un JMX</SubHeading>
            <p>
              Desde <strong>Inicio</strong> arrastra un archivo <Code>.jmx</Code> a la zona de
              importación (o haz clic para elegirlo). perfkit lo convierte al IR y muestra un{" "}
              <strong>reporte de fidelidad</strong>: una franja superior con cuántos elementos se
              migraron, cuántos necesitan revisión y el porcentaje de fidelidad. El importador{" "}
              <strong>nunca falla en silencio</strong>: cada elemento del JMX queda clasificado.
            </p>
            <Table
              cols={[
                { key: "estado", header: "Estado" },
                { key: "sig", header: "Qué significa" },
                { key: "accion", header: "Qué hacer" },
              ]}
              rows={[
                {
                  estado: <FidelityBadge status="migrated" />,
                  sig: "Migrado 1:1 a IR nativo.",
                  accion: "Nada, listo.",
                },
                {
                  estado: <FidelityBadge status="assisted" />,
                  sig: "Necesita revisión manual (p. ej. scripts Groovy/BeanShell).",
                  accion: "Lee la razón y aplica la sugerencia (extractor, variable, timer…).",
                },
                {
                  estado: <FidelityBadge status="unsupported" />,
                  sig: "No soportado por ahora (JDBC/JMS avanzado, plugins .jar…).",
                  accion: "Reemplaza con un equivalente declarativo o acota el alcance.",
                },
                {
                  estado: <FidelityBadge status="ignored" />,
                  sig: "Ignorado a propósito (listeners, cache manager…).",
                  accion: "Normalmente nada; verifica la razón.",
                },
              ]}
            />
            <p className="text-xs text-slate-500">
              ¿Sin un JMX a mano? Usa <Code>Abrir un ejemplo</Code> en Inicio para cargar un
              escenario de checkout de e-commerce.
            </p>
          </Section>

          {/* 3. Estructura del plan */}
          <Section id="estructura" title="Estructura del plan">
            <p>Un plan se organiza en tres niveles jerárquicos:</p>
            <DefList
              items={[
                {
                  term: "Escenario",
                  def: (
                    <>
                      La raíz. Define nombre, <strong>variables</strong> globales,{" "}
                      <strong>datasets</strong> CSV y <strong>defaults</strong> HTTP (URL base,
                      cabeceras, timeouts, follow-redirects).
                    </>
                  ),
                },
                {
                  term: "Grupos de hilos",
                  def: (
                    <>
                      Cada grupo modela una población de usuarios. Configura{" "}
                      <strong>VUs</strong> (usuarios virtuales), <strong>rampa</strong> de subida,
                      tiempo de <strong>hold</strong>, rampa de bajada y, como criterio de fin,{" "}
                      <strong>iteraciones</strong> o <strong>duración</strong>. También el
                      comportamiento ante error (continuar / detener hilo / detener prueba).
                    </>
                  ),
                },
                {
                  term: "Pasos",
                  def: (
                    <>
                      La secuencia que ejecuta cada VU: peticiones HTTP, controladores (transaction,
                      loop, if…) y timers. Los pasos pueden anidarse dentro de controladores.
                    </>
                  ),
                },
              ]}
            />
            <p>
              En la vista <strong>Plan</strong> el árbol de la izquierda muestra esta jerarquía y el
              panel derecho edita el elemento seleccionado. Todos los cambios son inmediatos sobre
              el plan en memoria.
            </p>
          </Section>

          {/* 4. Controladores */}
          <Section id="controladores" title="Tipos de controladores">
            <p>
              Los controladores agrupan y orquestan pasos. perfkit soporta de forma nativa los
              siguientes (más la petición HTTP, que es el paso base):
            </p>
            <Table
              cols={[
                { key: "tipo", header: "Tipo" },
                { key: "hace", header: "Qué hace" },
                { key: "cuando", header: "Cuándo usarlo" },
              ]}
              rows={[
                {
                  tipo: <strong>HTTP Request</strong>,
                  hace: "Petición HTTP/HTTPS: método, URL, query, cabeceras, body (raw o form).",
                  cuando: "La unidad de trabajo básica de cualquier prueba de API/web.",
                },
                {
                  tipo: <strong>Transaction</strong>,
                  hace: "Agrupa varios pasos y mide la transacción completa como una sola métrica.",
                  cuando: "Para medir un flujo de negocio (login, checkout) extremo a extremo.",
                },
                {
                  tipo: <strong>Loop</strong>,
                  hace: "Repite sus pasos un número fijo de veces.",
                  cuando: "Añadir N ítems al carrito, paginar un número conocido de páginas.",
                },
                {
                  tipo: <strong>If</strong>,
                  hace: "Ejecuta sus pasos solo si la condición es verdadera.",
                  cuando: "Ramificar según una variable extraída (p. ej. si hay token).",
                },
                {
                  tipo: <strong>While</strong>,
                  hace: "Repite mientras la condición se cumpla, con un tope de iteraciones.",
                  cuando: "Sondear hasta que un recurso esté listo (con límite de seguridad).",
                },
                {
                  tipo: (
                    <strong>
                      Throughput <span className="font-normal">(%)</span>
                    </strong>
                  ),
                  hace: "Ejecuta sus pasos solo en un porcentaje de las pasadas.",
                  cuando: "Que solo el 30% de los usuarios responda una encuesta, por ejemplo.",
                },
                {
                  tipo: <strong>Interleave</strong>,
                  hace: "Ejecuta un hijo distinto en cada pasada, de forma rotatoria.",
                  cuando: "Alternar endpoints equivalentes pasada tras pasada.",
                },
                {
                  tipo: <strong>Random</strong>,
                  hace: "Ejecuta un hijo elegido al azar en cada pasada.",
                  cuando: "Simular elecciones aleatorias entre variantes.",
                },
                {
                  tipo: <strong>Timer</strong>,
                  hace: (
                    <>
                      Introduce esperas (think-time) o regula el ritmo:{" "}
                      <Code>constant</Code>, <Code>uniform</Code>, <Code>gaussian</Code> y{" "}
                      <Code>constant_throughput</Code>.
                    </>
                  ),
                  cuando: "Hacer realista el comportamiento del usuario o fijar un ritmo objetivo.",
                },
                {
                  tipo: <strong>Kafka</strong>,
                  hace: "Publica un mensaje (clave/payload) en un topic de Kafka.",
                  cuando: "Probar productores de eventos junto a las peticiones HTTP.",
                },
              ]}
            />
          </Section>

          {/* 5. Assertions, extractores y variables */}
          <Section id="assertions" title="Assertions, extractores y variables">
            <SubHeading>Assertions (validar respuestas)</SubHeading>
            <p>
              Una assertion marca una petición como fallida si no se cumple. Disponibles:{" "}
              <strong>status</strong> (lista de códigos aceptados), <strong>body contains</strong> /{" "}
              <strong>body matches</strong> (subcadena o regex, con opción de negar),{" "}
              <strong>JSONPath</strong> (existe o igual a un valor), <strong>duration</strong>{" "}
              (latencia por debajo de un máximo) y <strong>size</strong> (tamaño por debajo de un
              máximo de bytes).
            </p>
            <SubHeading>Extractores (encadenar peticiones)</SubHeading>
            <p>
              Un extractor captura un valor de la respuesta y lo guarda en una variable para usarlo
              en pasos siguientes (correlación). Tipos: <strong>regex</strong> (patrón + grupo),{" "}
              <strong>JSONPath</strong> y <strong>boundary</strong> (texto entre un delimitador
              izquierdo y uno derecho). Todos admiten un valor por defecto si no hay coincidencia.
            </p>
            <SubHeading>Variables</SubHeading>
            <p>
              Las variables se referencian con la sintaxis <Code>{"${nombre}"}</Code> en URLs,
              cabeceras y cuerpos. Provienen de tres fuentes:
            </p>
            <DefList
              items={[
                {
                  term: "Globales",
                  def: "Definidas en el escenario (p. ej. base de URL, versión de API).",
                },
                {
                  term: "Datasets CSV",
                  def: "Una fila por iteración: usuario/contraseña, SKUs, etc. (con reciclado opcional).",
                },
                {
                  term: "Extraídas",
                  def: "Capturadas en tiempo de ejecución por un extractor (p. ej. un token de login).",
                },
              ]}
            />
            <p className="text-xs text-slate-500">
              Manejo de secretos: no escribas tokens en el plan. Pásalos por variables de entorno
              con prefijo <Code>PERFKIT_VAR_*</Code>; los logs y reportes redactan valores sensibles
              comunes.
            </p>
          </Section>

          {/* 6. Probar petición */}
          <Section id="probar" title="Probar petición e inspección">
            <p>
              Mientras construyes el plan puedes <strong>probar una sola petición</strong> sin
              lanzar una carga completa. Es una corrida de 1 VU y 1 iteración con captura activada:
              ideal para depurar una URL, una cabecera o un extractor.
            </p>
            <p>
              El resultado muestra el <strong>detalle de la petición</strong>: request y response
              (cabeceras y cuerpo), las <strong>variables</strong> disponibles en ese punto y los
              valores <strong>extraídos</strong>. Por seguridad, las cabeceras y variables
              sensibles (authorization, cookie, token, password) se <strong>redactan</strong> por
              defecto; un <strong>flag de texto plano</strong> permite verlas sin redactar solo en
              entornos de prueba.
            </p>
          </Section>

          {/* 7. Ejecutar */}
          <Section id="ejecutar" title="Ejecutar">
            <p>
              La vista <strong>Ejecutar</strong> lanza el escenario. Puedes aplicar{" "}
              <strong>overrides</strong> solo para esa corrida (no editan el plan):
            </p>
            <DefList
              items={[
                {
                  term: "URL base",
                  def: "Apunta el mismo plan a otro entorno (staging, local…).",
                },
                { term: "VUs", def: "Sobrescribe los usuarios virtuales de todos los grupos." },
                { term: "Duración", def: "Sobrescribe iteraciones/duración del plan, en segundos." },
              ]}
            />
            <p>
              Durante la ejecución verás un <strong>dashboard en vivo</strong>: VUs activos,
              requests, throughput, P95 y tasa de error, más gráficas en tiempo real de throughput,
              latencia P95 y usuarios virtuales. Al terminar saltas automáticamente al{" "}
              <strong>Reporte</strong>. La casilla <strong>Capturar peticiones</strong> guarda el
              detalle de cada petición para inspección (úsala en corridas cortas, no en carga real).
            </p>
          </Section>

          {/* 8. Reporte */}
          <Section id="reporte" title="Reporte">
            <p>El reporte se organiza en pestañas:</p>
            <Table
              cols={[
                { key: "tab", header: "Pestaña" },
                { key: "contenido", header: "Qué muestra" },
              ]}
              rows={[
                {
                  tab: <strong>Resumen</strong>,
                  contenido:
                    "KPIs globales (requests, throughput, error rate, percentiles) y la tabla por etiqueta/transacción.",
                },
                {
                  tab: <strong>Latencia</strong>,
                  contenido:
                    "Curva de percentiles, histograma de latencias, TTFB y Apdex.",
                },
                {
                  tab: <strong>Capacidad</strong>,
                  contenido:
                    "Saturación de VUs (throughput vs usuarios activos) y bytes transferidos.",
                },
                {
                  tab: <strong>Errores</strong>,
                  contenido: "Reparto por código de estado HTTP y por tipo de error.",
                },
                {
                  tab: <strong>SLA</strong>,
                  contenido:
                    "Umbrales editables y veredicto PASA/FALLA (el mismo criterio que el gate de CI).",
                },
                {
                  tab: <strong>Heatmap</strong>,
                  contenido: "Mapa de calor latencia × tiempo para ver degradaciones puntuales.",
                },
                {
                  tab: <strong>Peticiones</strong>,
                  contenido:
                    "Lista de peticiones capturadas (solo si activaste la captura) con su request/response.",
                },
              ]}
            />
            <SubHeading>Qué significan las métricas</SubHeading>
            <DefList
              items={[
                {
                  term: "p50 / p95 / p99",
                  def: "Percentiles de latencia: el 50%, 95% y 99% de las peticiones tardaron menos que ese valor. El p95/p99 captura la cola lenta (la experiencia de los usuarios peor servidos).",
                },
                {
                  term: "TTFB",
                  def: "Time To First Byte: tiempo hasta el primer byte de la respuesta (red + procesamiento del servidor antes de empezar a responder).",
                },
                {
                  term: "Apdex",
                  def: "Índice de satisfacción (0–1) según un umbral T: satisfechas ≤ T, tolerando ≤ 4T, el resto frustradas. Score = (satisfechas + tolerando/2) / total.",
                },
                {
                  term: "Throughput",
                  def: "Peticiones completadas por segundo (req/s): la capacidad efectiva observada.",
                },
                {
                  term: "Punto de saturación",
                  def: "El nivel de VUs a partir del cual añadir más usuarios ya no aumenta el throughput (y la latencia se dispara): el límite práctico del sistema.",
                },
              ]}
            />
          </Section>

          {/* 9. Importar/Exportar y migración */}
          <Section id="import-export" title="Importar / Exportar y migración desde JMeter">
            <p>perfkit interopera con varios formatos:</p>
            <Table
              cols={[
                { key: "fmt", header: "Formato" },
                { key: "uso", header: "Uso" },
              ]}
              rows={[
                {
                  fmt: <Code>YAML</Code>,
                  uso: "El IR canónico: legible y versionable en Git. Formato recomendado del plan.",
                },
                {
                  fmt: <Code>PKB</Code>,
                  uso: "Paquete perfkit (plan + recursos) para compartir un escenario completo.",
                },
                {
                  fmt: <Code>JMX</Code>,
                  uso: "Apache JMeter, para importar planes existentes y exportar de vuelta (round-trip).",
                },
                {
                  fmt: <Code>JSON</Code>,
                  uso: "El IR en JSON (interoperar con herramientas/scripts). Exportable también desde el navegador.",
                },
              ]}
            />
            <p>
              El <strong>round-trip a JMeter</strong> permite exportar un plan perfkit de vuelta a
              JMX. La importación clasifica cada elemento en cuatro niveles de fidelidad —{" "}
              <FidelityBadge status="migrated" /> <FidelityBadge status="assisted" />{" "}
              <FidelityBadge status="unsupported" /> <FidelityBadge status="ignored" /> — para que
              sepas exactamente qué se trasladó 1:1 y qué necesita tu atención.
            </p>
            <p className="text-xs text-slate-500">
              La exportación a YAML/JMX/PKB requiere la app nativa. En el navegador (demo) solo se
              descarga el JSON del plan client-side.
            </p>
          </Section>

          {/* 10. Histórico y SLA en CI */}
          <Section id="historico" title="Histórico y comparación · SLA en CI">
            <p>
              La vista <strong>Histórico</strong> guarda el resumen de cada ejecución, lo etiqueta
              (branch, entorno, build, commit) y lo compara contra un <strong>baseline</strong>: una
              corrida de referencia que fijas por escenario y entorno. La comparación muestra los
              deltas de P95, throughput y tasa de error, y marca <Badge color="red">REGRESIÓN</Badge>{" "}
              cuando el rendimiento empeora respecto al baseline. La gráfica de{" "}
              <strong>tendencia</strong> dibuja la evolución de una métrica a lo largo de las
              corridas.
            </p>
            <SubHeading>Quality gate en CI</SubHeading>
            <p>
              En CI defines umbrales y los evalúas con <Code>perfkit gate</Code> sobre el{" "}
              <Code>summary.json</Code> de un run. Si algún umbral no se cumple, el comando devuelve
              un <strong>exit code distinto de cero</strong> y el pipeline falla.
            </p>
            <CodeBlock>{`# thresholds.yaml
max_error_rate: 0.01        # 1% de error como máximo
max_p95_ms: 800             # p95 <= 800 ms
max_p99_ms: 1500            # p99 <= 1500 ms
min_throughput_per_sec: 50  # al menos 50 req/s

perfkit gate reports/run-001/summary.json --thresholds thresholds.yaml
# exit code != 0  =>  el pipeline falla por umbrales`}</CodeBlock>
            <p className="text-xs text-slate-500">
              La pestaña <strong>SLA</strong> del reporte usa exactamente este criterio, para que
              veas el veredicto antes de llevarlo a CI.
            </p>
          </Section>

          {/* 11. Avanzado */}
          <Section id="avanzado" title="Avanzado">
            <DefList
              items={[
                {
                  term: "Distribuido",
                  def: "Ejecución en cluster (coordinator/worker) para generar carga por encima de lo que da una sola máquina. Modelo propio de perfkit, posterior al MVP.",
                },
                {
                  term: "IA gobernada",
                  def: "Asistencia opcional (SaaS) para sugerencias sobre el plan y la migración. Viene desactivada por defecto (off) y no envía datos sin tu consentimiento.",
                },
                {
                  term: "Plugins WASM",
                  def: "Extensiones en WebAssembly firmadas y aisladas, para añadir capacidades sin comprometer la seguridad del host.",
                },
              ]}
            />
            <p className="text-xs text-slate-500">
              Estos componentes son posteriores al MVP y no bloquean el flujo local de importar,
              ejecutar y reportar.
            </p>
          </Section>

          {/* 12. Casos de uso */}
          <Section id="casos" title="Casos de uso">
            <Table
              cols={[
                { key: "caso", header: "Caso" },
                { key: "como", header: "Cómo" },
              ]}
              rows={[
                {
                  caso: <strong>Smoke test</strong>,
                  como: "1–2 VUs durante pocos segundos para confirmar que el flujo funciona antes de cargar.",
                },
                {
                  caso: <strong>Prueba de carga</strong>,
                  como: "Sube VUs con rampa y mantén el hold; observa throughput, P95 y el punto de saturación.",
                },
                {
                  caso: <strong>Baseline en CI</strong>,
                  como: "Guarda la corrida, fíjala como baseline y deja que perfkit gate falle el pipeline ante regresiones.",
                },
                {
                  caso: <strong>Comparar entornos</strong>,
                  como: "Ejecuta el mismo plan con override de URL base (staging vs prod-like) y compara los reportes.",
                },
                {
                  caso: <strong>Depurar un endpoint</strong>,
                  como: "Usa Probar petición con captura para inspeccionar request/response y validar extractores.",
                },
              ]}
            />
          </Section>
        </article>
      </div>
    </div>
  );
};
