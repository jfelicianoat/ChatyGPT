# Cierre de huecos de la Fase 3

Fecha: 2026-08-01.

La Fase 3 se declaró completada con GPTs versionados, seleccionables y
transportables, pero varios elementos que el encargo pedía explícitamente no
existían: el historial de versiones no era consultable, no se podía restaurar
una revisión anterior, no había duplicación y el modelo preferido era un campo
muerto. Este documento recoge su cierre.

## Matriz del corte

| Requisito | Estado | Evidencia | Resultado | Manual pendiente |
|---|---|---|---|---|
| Historial de versiones | Verificado automáticamente | `list_custom_gpt_versions` y botón **Historial** en cada ficha | muestra todas las revisiones con su contenido, permisos, modelo preferido y cuántas respuestas quedaron congeladas con cada una | editar un GPT dos veces y abrir su historial |
| Restaurar una versión anterior | Verificado automáticamente | `restore_custom_gpt_version` y test `custom_gpt_history_restores_a_previous_version_without_losing_any` | **crea una versión nueva** con el contenido de la elegida; no revive la fila antigua ni borra nada, así las respuestas ya emitidas conservan intacta la suya | restaurar la versión 1 y comprobar que aparece como versión 3 |
| Confirmación al restaurar | Verificado automáticamente | mismo test | sin `confirmed` devuelve `Validation`; restaurar la versión ya activa devuelve `Conflict` | pulsar **Restaurar** y revisar el aviso |
| Permisos que acompañan a la restauración | Verificado automáticamente | copia de `gpt_tool_permissions` en la misma transacción | restaurar recupera el GPT tal como era, no una versión desarmada | restaurar una versión con Renombrar en `confirm` |
| Duplicar un GPT | Verificado automáticamente | `duplicate_custom_gpt` y test `duplicating_a_custom_gpt_never_carries_permissions_or_knowledge` | la copia hereda instrucciones, iniciadores, modelo y proyecto, pero **nunca** permisos ni conocimiento; empieza en su versión 1 | pulsar **Duplicar** y revisar los dos indicadores de permiso |
| Modelo preferido | Verificado automáticamente | `validated_preferred_model`, campo en el formulario y `model_requirements.preferred_model` | se envía al Broker solo si el GPT lo define, con `fallback_allowed` activo | fijar un modelo local y comprobar el proveedor bajo la respuesta |
| Modelo preferido congelado por tarea | Verificado por diseño y compilación | `CustomGptContext.preferred_model` viaja con la versión congelada | cambiar el modelo del GPT no altera peticiones ya construidas | editar el GPT tras enviar y abrir **Ver contexto utilizado** |
| Proyecto predeterminado | Verificado por compilación y auditoría | `custom_gpts.default_project_id` y `conversation.default_project_applied` | se aplica **solo** a chats sin proyecto; nunca mueve uno ya clasificado | elegir el GPT en un chat suelto y en otro ya asignado |
| Límite real del Broker | Verificado automáticamente | test `preferred_model_is_validated_against_the_broker_limit` | 128 caracteres y sin espacios, igual que `ModelRequirements` del Broker | intentar guardar un modelo con espacios |
| Vista previa sin coste | Verificado automáticamente | comando `preview_custom_gpt` y test `the_preview_block_is_literally_the_one_sent_to_the_broker` | muestra el bloque exacto que se antepone al mensaje, los permisos, el modelo, el proyecto, el recuento de conocimiento y archivos, y avisos accionables; no crea ninguna tarea | pulsar **Vista previa** en una ficha de GPT |
| Vista previa que no puede mentir | Verificado automáticamente | `custom_gpt_prompt_block` compartida entre la vista previa y la petición | el test comprueba que el bloque mostrado aparece **literalmente** dentro del prompt real | comparar la vista previa con **Ver contexto utilizado** tras enviar |
| Prueba desde la ficha | Verificado automáticamente | botón **Probar**, diálogo específico y prueba `prueba un GPT en un chat real que queda guardado` | crea una conversación normal `Prueba · nombre`, aplica el proyecto predeterminado, asocia el GPT y envía la pregunta por el recorrido habitual; el resultado o el error permanecen en **Recientes** | **Inicio → GPTs → Probar**, escribir una pregunta y comprobar que se abre el chat de prueba |

## Contrato comprobado, no supuesto

`preferred_model` no estaba en el fixture local `contracts/broker/2.7`. Antes de
enviarlo se comprobó el código real del Broker:

```
app/schemas.py:170  class ModelRequirements(StrictBaseModel):
app/schemas.py:171      preferred_model: str | None = Field(default=None, max_length=128)
```

De ahí salen el nombre exacto del campo y el límite de 128 caracteres que valida
la aplicación antes de guardar.

## Verificación ejecutada

```powershell
cargo fmt --manifest-path apps\desktop\src-tauri\Cargo.toml
cargo test --manifest-path apps\desktop\src-tauri\Cargo.toml
cargo clippy --manifest-path apps\desktop\src-tauri\Cargo.toml --all-targets -- -D warnings
.\node_modules\.bin\tsc.CMD -b --pretty false
.\node_modules\.bin\vitest.CMD run
python -m unittest discover -s tests
git diff --check
```

Resultado: 103 pruebas de Rust (eran 99), 65 de TypeScript (eran 63) y 17 de
Python en verde; clippy sin avisos. La cobertura de líneas de Rust queda en
71,12 %, por encima del umbral de 70 que aplica la CI.

## Decisiones

- **Restaurar crea, no revive.** Reactivar la fila antigua habría hecho que el
  historial mintiera sobre el orden real de los cambios. Crear una versión nueva
  con el contenido recuperado mantiene el registro fiel y deja intactas las
  tareas que apuntan a versiones anteriores.
- **Un duplicado nace sin permisos ni conocimiento**, igual que un GPT
  importado. Copiar un asistente no puede ser una vía silenciosa de propagar
  accesos concedidos o datos sensibles.
- **El modelo es una preferencia.** Se envía junto a `fallback_allowed: true`,
  de modo que un modelo apagado no deja la conversación sin respuesta.
- **El proyecto predeterminado no reorganiza nada.** Solo actúa sobre chats que
  aún no pertenecen a ningún proyecto, y deja constancia cuando lo hace.
- **La vista previa comparte código con la petición real.** El bloque que se
  antepone al mensaje lo genera una única función, `custom_gpt_prompt_block`. Si
  cada camino construyera su propio texto, la vista previa dejaría de demostrar
  nada en cuanto ambos divergieran; el test lo comprueba exigiendo que el bloque
  mostrado aparezca literalmente dentro del prompt enviado.
- **La vista previa avisa de lo que hoy no funcionaría**: conocimiento
  desactivado o sin indexar, archivos aún no preparados, datos sensibles que
  obligan a permanecer en local y un proyecto predeterminado que ya no existe.

## Lo que sigue faltando en la Fase 3

- **Funciones definidas por el cliente**: sigue existiendo una única herramienta
  codificada (`rename_conversation`). El encargo pide que cada GPT pueda definir
  las suyas, lo que exige un modelo de validación y confirmación propio.
- **Matriz de permisos**: sigue con 2 capacidades de las 8 del encargo. Las
  restantes (ejecutar scripts, leer carpetas autorizadas, modificar archivos,
  llamar APIs externas, crear tareas programadas) no se han añadido a propósito:
  declarar permisos para herramientas que no existen sería teatro de seguridad.
  Ahora que las carpetas autorizadas están activas, «leer carpetas» es el
  siguiente candidato razonable.
- **Límites y configuración avanzada de contexto por GPT**: el icono y el perfil
  de ejecución ya están versionados, pero todavía no existe un presupuesto de
  contexto propio más granular que las opciones generales del perfil.
