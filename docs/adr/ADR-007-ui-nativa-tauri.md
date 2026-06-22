# ADR-007: UI nativa con Tauri 2

- **Estado:** Aceptado
- **Fecha:** 2026-06-19
- **Decisores:** frontend-ux-lead, platform-architect

## Contexto

El MVP necesita una UI operativa para QA que permita importar un JMX, ver el árbol
del plan, editar lo esencial (HTTP samplers, timers, assertions, datasets),
ejecutar localmente y leer el reporte y la fidelidad de migración (plan §6.6, §9
Fase 3). Reglas de UX: nada de landing page, la primera pantalla debe ser
operativa, y **el QA debe poder trabajar sin escribir TypeScript** (la vista YAML
es útil pero no obligatoria). La herramienta es de escritorio y local, y el motor
ya vive en crates Rust (ADR-001).

## Decisión

La UI es una **aplicación NATIVA de escritorio con Tauri 2** y frontend **React +
TypeScript + Tailwind** (`ui/src-tauri`, proyecto Cargo aparte excluido del
workspace — ver ADR-001).

### El shell Rust llama directamente a los crates del engine

El shell de Tauri (el lado Rust) **invoca directamente los crates** (`scenario-ir`,
`jmx-importer`, `engine`, `metrics`, `reports`) por path. **No hay servidor HTTP
intermedio**: importar, validar, ejecutar y reportar son llamadas in-process desde
el shell a las funciones del core. Esto reduce latencia, superficie de ataque y
complejidad de despliegue (un solo binario de escritorio).

### Métricas en vivo por eventos Tauri

Durante un run, el engine emite snapshots (`LiveSnapshot`) que el shell reenvía al
frontend mediante el **sistema de eventos de Tauri** (no polling HTTP). El
dashboard en vivo se actualiza por estos eventos.

### El IR es el contrato de la UI

La UI **edita el IR** (YAML), no un formato propio. La vista YAML existe pero no es
obligatoria: el QA opera con formularios.

### Aclaración importante sobre el rol del QA

TypeScript es **únicamente el lenguaje de implementación del frontend**. El usuario
QA **nunca escribe TypeScript** para usar la herramienta: trabaja con la UI gráfica
sobre el IR. Esto es coherente con §2.6 (la UI edita el IR; el QA no depende de
TypeScript) y con el DSL TS diferido y opcional de §2.7.

## Consecuencias

**Positivas**

- Un único binario de escritorio, sin servidor que desplegar ni puerto que exponer.
- Llamadas in-process al engine: baja latencia y menos superficie de ataque.
- Métricas en vivo por eventos nativos, sin polling.
- React + Tailwind permiten una estética moderna y flexible difícil de lograr con
  toolkits inmediatos de Rust.

**Negativas / costos**

- El proyecto Tauri queda fuera del workspace y depende de los crates por path; hay
  que evitar que sus versiones se desincronicen del core (ADR-001).
- Tauri exige dependencias de sistema (webview) en el entorno de build de la UI.
- Mezcla dos lenguajes (Rust + TS); el límite shell↔frontend debe mantenerse claro.

## Alternativas consideradas

- **egui / iced (UI 100% Rust):** descartadas; se pierde estética y flexibilidad de
  layout frente a HTML/CSS/React, clave para una UI moderna y familiar para QA.
- **Electron:** descartado por peor performance y mayor tamaño de binario frente a
  Tauri (que reutiliza el webview del sistema).
- **UI web servida por un servidor HTTP local:** descartada para el MVP; añade un
  servidor, un puerto y superficie de ataque innecesarios para una app local. Las
  llamadas in-process del shell Tauri son más simples y seguras.
- **Obligar a editar YAML a mano:** descartado por §6.6; la edición debe ser por
  formularios, con YAML como opción avanzada.
