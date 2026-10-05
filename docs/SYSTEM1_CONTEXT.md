# Contexto conversacional con System 1

Revisión: **3 de octubre de 2026** (ChatyGPT 0.4.0, esquema 26), incluido el
cambio de `Client_API.md` de esa fecha y las mediciones contra el broker real.

## Arquitectura y alcance

ChatyGPT conserva su ensamblador de `task_runtime/peticiones.rs`, los presupuestos
Enfocado/Equilibrado/Amplio, SQLite, las búsquedas de memoria y la selección
documental existentes. No hay otra memoria ni otro orquestador.

La selección se integra antes de preparar el envío normal y después de recuperar
los candidatos de una búsqueda semántica, también cuando esta se retoma al
arrancar. En modo aplicado, la petición persistida y la instantánea de fuentes
contienen la selección que realmente se envía. En modo sombra la petición no
cambia y la propuesta se evalúa después (véase «Latencia»).

System 1 puntúa hechos de memoria y fragmentos documentales opcionales. Se
conservan siempre la ventana reciente completa, la petición actual, los resúmenes
aprobados, las instrucciones de proyecto/GPT, los permisos, las preferencias e
instrucciones de memoria, los resultados de herramientas del flujo existente y
las vistas globales deliberadas de documentos. Cada archivo mantiene al menos
un fragmento: retirar todos haría que el ensamblador enviara el archivo entero.

No se recupera contenido nuevo de otros proyectos ni se amplían permisos.
Los candidatos son los que el mecanismo actual ya estaba autorizado a usar.
Si el contexto cabe en el presupuesto o no tiene candidatos opcionales, no se
solicitan juicios. Los candidatos que exceden el máximo de evaluaciones se
conservan. El historial reciente se protege porque puede contener restricciones,
referentes o criterios activos que no están representados en campos separados.

## Contrato con el broker

Se usa exclusivamente `POST /api/v1/system1/judge`, con la URL y la credencial
recargable del cliente existente. Se exige `capabilities.system1_judgments`;
su ausencia en versiones anteriores equivale a `false`. No hay llamadas
directas a Ollama, LAYA o LM Studio.

La petición envía:

- `use_case: "chatygpt_context_relevance"`;
- `threshold_profile: "default"`, obligatorio para este caso propio con la
  actualización del 3 de octubre;
- `decision_type: "score"` con una rúbrica de **tres niveles**: irrelevante,
  posiblemente útil o referente ambiguo, necesario o claramente relevante;
- instrucciones explícitas para puntuar relevancia y resolución de referentes;
- `input` con la petición actual, la ventana reciente (sin repetir la petición
  actual, que ya va en `current_request`) y un candidato;
- `cloud_allowed: false`.

Cada nivel se traduce a un valor representativo de su banda de umbrales:
0 → 0,0, 1 → 0,70 y 2 → 1,0. Así los umbrales configurables siguen decidiendo
qué hace cada nivel. Se valida el caso de uso, el tipo numérico, el índice
entero dentro de la rúbrica, la confianza y la latencia. `confidence` no se trata
como relevancia ni como probabilidad calibrada.

**Por qué tres niveles y no once.** Medido contra Nimble el 3 de octubre con 12
casos etiquetados: con la escala 0-10 solo devolvía 0 o 10 y aceptó 7 de 12
juicios; cinco de los seis candidatos relevantes salían `LOW_CONFIDENCE`. Con
tres niveles aceptó 10 de 12, todos correctos y sin descartar ningún relevante.
La muestra es pequeña; sirve para elegir la rúbrica, no para calibrar umbrales.

El proveedor, modelo, orden de fallback y umbrales del broker son configuración
del operador. ChatyGPT no envía `target`, ni elige jueces, ni interpreta sus
intentos como tareas que haya que sondear. Solo usa un juicio HTTP 200 con
`accepted: true`; el umbral de confianza lo aplica el broker (§15.5) y ChatyGPT
no lo duplica. No reintenta juicios rechazados.

La ampliación `system1_evaluation`, `target`, `attempts[].score_source` y notas
en bruto de §15.8 sirve para evaluación. Se toleran esos campos adicionales,
pero no gobiernan la selección: aunque un intento traiga `decision: 0` y
confianza alta, un rechazo de primer nivel nunca se usa como decisión.

## Selección, rechazos y recuperación

Los juicios se hacen por candidato porque el contrato no ofrece un batch de
relevancias. La operación completa tiene un plazo acotado.

**Un juicio rechazado solo protege a su candidato.** `LOW_CONFIDENCE`,
`INSUFFICIENT_MARGIN`, `INPUT_TOO_LARGE`, `SELF_REPORTED_SCORE` o un código
desconocido con `accepted: false` conservan ese candidato, que es la alternativa
que pide §15.4, y los demás juicios siguen valiendo. Medido en vivo, Nimble
rechaza por confianza justamente los referentes implícitos («haz lo mismo con
Athena»): con la política anterior, de todo o nada, cualquier candidato relevante
anulaba la selección entera y no se reducía nada.

**Fallback completo** (se conserva el ensamblado anterior tal cual):

- error de transporte, HTTP o salida que incumple el contrato;
- plazo agotado;
- broker sin `system1_judgments` o capacidades no disponibles;
- errores de configuración del broker, que se repetirían con cada candidato:
  `UNKNOWN_USE_CASE`, `UNKNOWN_THRESHOLD_PROFILE`, `MISSING_INSTRUCTIONS`,
  `SYSTEM1_DISABLED`;
- baja confianza global: ningún juicio de la selección fue aceptado;
- configuración local incoherente.

Con los valores iniciales, relevancia ≥ 0,80 conserva el candidato; < 0,55 lo
descarta; el tramo intermedio (nivel 1) se conserva si hay presupuesto o una
señal de referencia implícita. Esa señal busca **palabras completas** («lo
mismo», «segunda opción», «eso», «otro modelo»…): «proceso» o «peso» ya no la
activan. Solo hace la conservación más prudente; el juez evalúa el referente
mediante el historial. No se usa System 1 para resumir.

El presupuesto nuevo es un objetivo de priorización. Nunca se fuerza su
cumplimiento eliminando contenido obligatorio, relevante, rechazado o no
evaluado. Los límites de mensajes, caracteres y fragmentos del perfil de GPT
siguen siendo los del ensamblador original. Si ese mínimo protegido excede el
objetivo, se conserva y se refleja su tamaño real estimado en la traza.

## Privacidad

La clasificación de privacidad se calcula antes del filtro. Excluir una memoria
sensible no rebaja `local_only` ni reactiva las invocaciones auxiliares de la
tarea. Los turnos `confidential` y `local_only` también se juzgan: los modelos
System 1 de esta instalación (Nimble y LAYA) son locales, y el juicio siempre
envía `cloud_allowed: false`, que impide al broker usar un proveedor `egress`
o un Ollama remoto (§15.1). Es una decisión del propietario del sistema.

## Latencia

- **Juicios en paralelo**, en tandas de cuatro. El turno espera la tanda más
  lenta, no la suma. Contra el broker real: 12 juicios en 5,7 s.
- **Modo sombra fuera del turno.** La sombra no cambia nada de lo enviado, así
  que el turno se crea y se envía sin esperar. La selección se evalúa después en
  segundo plano y su traza se guarda en `broker_tasks.system1_trace_json`
  (migración `0026`). No se escribe en `request_json` porque ese cuerpo es la
  petición idempotente ya enviada o pendiente de reenvío.
- En modo aplicado los juicios sí preceden al envío, porque deciden qué se manda.

Medido en vivo: un juicio tarda entre 0,6 y 3,7 s según el tamaño del historial.
Con unos 40 000 caracteres de historial el broker responde `INPUT_TOO_LARGE`, y
ese candidato se conserva.

## Configuración y activación

Se lee al arrancar, siguiendo las variables de entorno del cliente. Reinicia
ChatyGPT después de cambiarlas.

| Variable | Valor inicial | Uso |
| --- | --- | --- |
| `CHATYGPT_SYSTEM1_CONTEXT_ENABLED` | `false` | Habilita la evaluación |
| `CHATYGPT_SYSTEM1_CONTEXT_SHADOW` | `true` | Evalúa y registra propuestas fuera del turno, conserva las fuentes |
| `CHATYGPT_SYSTEM1_CONTEXT_TIMEOUT_MS` | `75000` | Plazo HTTP y máximo total de la selección |
| `CHATYGPT_SYSTEM1_CONTEXT_INCLUDE_THRESHOLD` | `0.80` | Relevancia suficiente para conservar |
| `CHATYGPT_SYSTEM1_CONTEXT_UNCERTAIN_THRESHOLD` | `0.55` | Inicio del tramo incierto |
| `CHATYGPT_SYSTEM1_CONTEXT_MAX_TOKENS` | `6000` | Objetivo de contexto y activación por tamaño |
| `CHATYGPT_SYSTEM1_CONTEXT_MAX_CANDIDATES` | `12` | Máximo de juicios por selección |

`CHATYGPT_SYSTEM1_CONTEXT_MIN_CONFIDENCE` ya no existe: repetía el umbral del
broker y bajarlo no tenía efecto. Si sigue definida, se ignora.

Para observar el filtro, inicia el lanzador habitual desde PowerShell con:

```powershell
$env:CHATYGPT_SYSTEM1_CONTEXT_ENABLED = 'true'
$env:CHATYGPT_SYSTEM1_CONTEXT_SHADOW = 'true'
& '.\Arrancar ChatyGPT.bat'
```

Para aplicar los descartes, cambia `SHADOW` a `false` en ese mismo entorno y
reinicia. Para volver al recorrido anterior, establece `ENABLED` en `false`.
Una configuración incoherente desactiva la selección. Ninguna variable de
ChatyGPT cambia los proveedores o la política del broker.

## Observabilidad

En modo aplicado, `content.metadata.system1_context` queda guardado con la
petición enviada; en sombra, la traza está en `system1_trace_json`. Las dos
incluyen modo, motivo, tokens candidatos, tamaño antes/después, incluidos y
excluidos, propuesta en sombra, latencia, fallback y señal de referencia. Cada
candidato juzgado conserva ID, tipo, relevancia, confianza, indicador de
calibración, proveedor y, si su juicio fue rechazado, el motivo (`rejected`). No
se duplica el texto del candidato.

El panel **Contexto utilizado** muestra el resultado y la estimación del texto
enviado; la propuesta en sombra aparece cuando termina su evaluación. El evento
`system1.context_selection` del registro estructurado contiene recuentos,
códigos y banderas, nunca prompts, credenciales o títulos. La traza de fuente
semántica conserva su puntuación original de similitud; la relevancia System 1
es una señal independiente en la traza de selección.

Los tokens son **estimaciones** `ceil(caracteres Unicode del prompt / 4)`.
No son recuentos del tokenizador ni consumo facturado. En sombra, la propuesta
de tamaño es aproximada; el texto realmente enviado se mide sin recortarlo.

## Cinco conversaciones verificadas

Se ejecuta el ensamblador real y se compara el texto con el filtro desactivado.
Los juicios y las respuestas de esta tabla son **simulados**, con la forma que
devuelve el broker real; no demuestran por sí solos la precisión de Nimble/LAYA.
Los datos reproducibles están en [SYSTEM1_CONVERSATIONS.json](SYSTEM1_CONVERSATIONS.json)
(se regeneran con `CHATYGPT_SYSTEM1_TEST_EVIDENCE=<ruta>` y
`cargo test system1_five_conversations`).

Todos los casos conservan petición, historial, instrucciones de proyecto y la
restricción «Mantén la privacidad y pide permiso antes de publicar». Los dos
candidatos opcionales son el diseño relacionado y un recuerdo de trading.

| Caso / mensaje actual | Juicios | Contexto final opcional | Respuesta simulada, igual con feature off | Tokens estimados off → on |
| --- | --- | --- | --- | --- |
| «Haz lo mismo con Athena» | Athena nivel 2; trading nivel 0 | Athena | Aplicaré Athena con el diseño acordado, conservando la privacidad y sin publicar. | 1036 → 686 |
| «Usa la segunda opción» | Ollama local nivel 2; trading nivel 0 | Ollama local; el historial conserva «1. LAYA remoto; 2. Ollama local» | Aplicaré Ollama local con el diseño acordado, conservando la privacidad y sin publicar. | 1037 → 687 |
| «Configura Athena siguiendo la propuesta anterior» | Athena nivel 2; trading nivel 0 | Athena | Aplicaré Athena con el diseño acordado, conservando la privacidad y sin publicar. | 1035 → 686 |
| «Explica Athena», contexto dentro del presupuesto | Sin juicios | Athena y trading | Aplicaré Athena con el diseño acordado, conservando la privacidad y sin publicar. | 1019 → 1019 |
| «Aplica eso al Broker» | Broker `LOW_CONFIDENCE` (rechazado); trading nivel 0 | Broker, conservado por su rechazo; trading retirado | Aplicaré Broker con el diseño acordado, conservando la privacidad y sin publicar. | 1033 → 683 |

Los cuatro descartes reducen el prompt estimado en torno al 34 % en estos datos.

## Comprobación contra el broker real

El 3 de octubre, contra AI Broker 2.11 (`192.168.1.52:8765`, Nimble como
primario), con la rúbrica y las instrucciones exactas del código y tandas de
cuatro juicios en paralelo, sobre 12 casos etiquetados (6 relevantes, 6 no):

- 10 juicios aceptados, todos correctos; ningún candidato relevante descartado;
- los 6 irrelevantes, retirados con confianza 0,94-0,99;
- los dos referentes implícitos de «Haz lo mismo con Athena» salieron
  `LOW_CONFIDENCE` y se conservaron por su propio rechazo;
- 12 juicios en 5,7 s.

## Verificación ejecutada

- Rust: **393 pruebas aprobadas**, incluidas **20 específicas de System 1**.
- `cargo fmt --check` y `cargo clippy --all-targets -- -D warnings`: limpios.
- Interfaz: **354 pruebas aprobadas** en 34 archivos; comprobación de tipos aprobada.
- Contratos y lanzadores: **44 pruebas Python aprobadas**.
- Vite: compilación de producción aprobada.
- Ejecutable de escritorio 0.4.0 regenerado con `tauri build --no-bundle`.

## Archivos y límites

- `broker/contracts.rs`, `broker/mod.rs`, `broker/system1.rs`: capacidades,
  configuración, rúbrica y transporte autenticado del juicio.
- `broker/simulated.rs`: respuestas por contenido (`respond_when`) para probar
  juicios concurrentes sin depender del orden de llegada.
- `system1.rs`, `system1/tests.rs`: selección, rechazos por candidato, tandas
  paralelas, sombra en segundo plano, configuración e instrumentación.
- `task_runtime.rs`, `task_runtime/peticiones.rs`, `task_runtime/herramientas.rs`,
  `task_runtime/tests_system1.rs`: integración en ambos recorridos y evidencias.
- `migrations/0026_system1_shadow_trace.sql`, `db/contexto.rs`,
  `db/tipos/memoria.rs`: traza en sombra y su exposición, sin memoria nueva.
- `domain/memoria.ts`, `App.tsx`: presentación de la selección y versión visible
  en la barra lateral.

Queda pendiente calibrar los umbrales con conversaciones reales y medir los
tokens de los propios juicios. El filtro permanece desactivado de fábrica y
dispone de modo sombra para esa evaluación.
