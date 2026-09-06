# Compatibilidad de ChatyGPT con AI Broker

Revisión: **6 de septiembre de 2026** (contrato 2.10).

Este documento describe al cliente ChatyGPT. No es la especificación de AI Broker ni
autoriza cambios en ese proyecto.

## Regla de compatibilidad

ChatyGPT valida de forma estricta los campos que necesita y tolera campos adicionales. El
cuerpo de petición sigue el baseline 2.8; el lector admite las extensiones aditivas 2.9
y 2.10.
No se compara la versión como una cadena para conceder capacidades: se usan las banderas y
campos anunciados por `/api/v1/capabilities` y la creación real de la tarea sigue siendo la
autoridad final.

| Área | Comportamiento actual |
| --- | --- |
| Creación | `POST /api/v1/tasks` con clave idempotente y persistencia local previa |
| Estado | Polling de estados no terminales, incluidas esperas de memoria, herramientas y dependencias |
| Resultado | Lee `assistant_content` y mantiene `result_markdown` para tareas anteriores |
| Identidad 2.9 | Lee `served_by`, `models_used` y `fallback_used` si existen |
| Ingesta | Negocia formatos y límites antes de seleccionar o subir archivos |
| Sandbox | Solo se habilita por turno tras comprobar capacidad y confirmación |
| Dependencias | Usa grupos estables para lotes de embeddings cuando el Broker los anuncia |
| Credencial | Envía `x-admin-token` desde Rust; una rotación es recuperable |
| Exclusividad 2.10 | `auxiliary_invocations: false` en conversaciones `confidential` o `local_only` (§8.4) |
| Evidencia 2.10 | Al completarse una tarea registra qué invocaciones eran suyas y qué poda se aplicó (§8.1 y §8.5) |
| Artefactos 2.10 | Lista y descarga los ficheros que produce una tarea, con `final: true` como entregable (§8.3) |

ChatyGPT no envía `exclude_from_model_learning`: las conversaciones reales sí forman parte
del tráfico normal del producto. Esa bandera pertenece a clientes de evaluación como
Model_Drift.

## Contrato 2.10 (septiembre de 2026)

**Exclusividad de contenido (§8.4).** El Broker mide su catálogo invocando a otro
modelo con el mismo prompt, bajo el mismo `task_id` y cuando la tarea ya ha
terminado. Con una conversación marcada `confidential` o `local_only` ese sondeo
se queda en modelos locales, pero «local» no es «el modelo que atiende esta
conversación», y quien pidió privacidad no pidió eso. Esas conversaciones envían
`auxiliary_invocations: false`.

El campo **se retira antes de salir** si el Broker no anuncia
`auxiliary_invocations_optout`. La validación del Broker es estricta
(`extra="forbid"`): un campo que no exista en su contrato hace fallar la petición
entera con 422. Perder la garantía adicional es malo; perder el mensaje de la
persona por haberla pedido es peor.

**Evidencia de ejecución (§8.1 y §8.5).** Al completarse una tarea se registra
—en el log, no en la interfaz— cuántas invocaciones eran de la tarea y cuántas
del Broker, con qué roles, y qué poda se aplicó de verdad al prompt. La
separación usa el booleano `contractual`, nunca el nombre del rol: esa lista
crece y se rompe en silencio.

**Artefactos (§8.3).** `list_task_artifacts` y `save_task_artifact` recogen los
ficheros que produce una tarea. El entregable se identifica por `final: true`, no
por `artifact_type`, y un `410` al descargar se distingue de un `404`: el fichero
existió y se borró a propósito.

**Limitación conocida.** Las imágenes que devuelve un modelo llegan como
artefactos `image_output` y la interfaz todavía no las pinta. Se registran en el
log al completarse la tarea (`task.image_artifacts_pending`) y se pueden recoger
con `save_task_artifact`, pero no aparecen solas en la conversación.

**El diagnóstico lo dice.** `content_exclusivity` y `demonstrable_execution` en
`BrokerDiagnostic` responden si este Broker puede prometer esas dos cosas.
`null` es «no consta» —no se pudieron leer las capacidades—, que no es `false`.

**Token de sesión (§3.1).** `scripts/verify_broker.py` lee
`ai-broker/session_admin_token` antes que `dashboard_admin_token`. La primera se
regenera en cada arranque del Broker; la segunda es del operador, para fijar un
token estable, y el contrato pide explícitamente no leerla en su lugar. Con el
orden anterior, un token del operador caducado ganaba a una sesión válida y el
síntoma era un 403 que parecía un fallo de integración.

**La verificación compara un mínimo, no una igualdad.** `verify_broker.py`
exigía `contract_version == "2.8"` y fallaba contra un 2.10 antes de comprobar
nada más. El contrato crece de forma aditiva, así que se compara contra
`MINIMUM_CONTRACT_VERSION` por número — y por número, porque `"2.10" < "2.9"` es
cierto entre cadenas y falso entre versiones.

Los fixtures bajo `contracts/broker/2.5` a `2.8` son capturas históricas y pruebas de
compatibilidad del cliente. El soporte de campos 2.9 está tipado y probado en
`apps/desktop/src-tauri/src/broker/contracts.rs`; la ausencia de una carpeta de fixture 2.9
no significa que el lector rechace esa revisión.

