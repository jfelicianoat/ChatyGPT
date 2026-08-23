# Endurecimiento de Fase 0

> **Documento histórico de fase.** Registra decisiones y pruebas de este corte; el estado
> vigente está en [CURRENT_STATE.md](CURRENT_STATE.md).

Fecha de inicio: 2026-08-01.

La Fase 0 se declaró operativa con la aplicación arrancando, migrando y
recuperando tareas, pero su bloque de seguridad y observabilidad (0D) y el de
calidad (0E) quedaron abiertos. Este documento recoge el cierre de esa deuda,
punto por punto, con las pruebas realmente ejecutadas.

## 1. Registro estructurado con correlación y redacción

Estado: **implementado y verificado automáticamente**.

### Decisión

El riesgo declarado en la arquitectura era «secretos en logs/DB». La solución
habitual —redactar por nombre de clave— falla en cuanto alguien añade un campo
nuevo con otro nombre. Aquí se ha invertido el problema: el registro **no tiene
un tipo de dato para texto libre**. `FieldValue` solo admite

- `Count`: recuentos y códigos HTTP;
- `Flag`: banderas;
- `Id`: identificadores internos (`[A-Za-z0-9_-]`, máximo 64);
- `Code`: vocabulario controlado en minúsculas (máximo 32);
- `Millis`: duraciones.

Cualquier valor que no cumpla el formato se sustituye por `[redactado]` en vez
de escribirse. Un prompt, una ruta de Windows o un token largo incumplen esas
reglas por construcción.

### Qué se registra

| Evento | Correlación | Campos |
|---|---|---|
| `app.started` | id de arranque | `schema_version`, `recovered_tasks`, `recovered_attachments` |
| `app.database_failed` | id de arranque | `error_kind` |
| `broker.diagnosed` | — | `reachable`, `ready`, `latency_ms` |
| `broker.transport_failed` | — | `operation` |
| `broker.response_rejected` | — | `operation`, `status` |
| `broker.contract_mismatch` | — | `operation` |
| `task.submitted` | `local_task_id` | `remote_task_id`, `status` |
| `task.submit_retry` | `local_task_id` | `error_kind` |
| `task.orphaned` | `local_task_id` | `phase`, `error_kind` |
| `task.state_settled` | `local_task_id` | `status`, `polls` |
| `task.poll_error` | `local_task_id` | `error_kind` |
| `task.cancel_requested` | `local_task_id` | `status` |

`task.submitted` es el punto que enlaza la identidad local con la remota, que es
lo que permite reconstruir después qué tarea del Broker atendió cada turno.

De los errores se registra su **clase** (`error_kind`), nunca su mensaje: el
texto de un error del Broker puede citar el contenido enviado.

### Destino y rotación

Una línea JSON por evento en `<AppLocalData>/logs/chatygpt.log`, con marca de
tiempo UTC ISO-8601. Al superar 1 MiB se rota a `chatygpt.log.1` y se conserva
una única copia previa, por lo que el registro no puede crecer sin límite. La
ruta activa se expone en el arranque y aparece como información emergente del
pie de la barra lateral.

Un fallo de escritura del registro nunca interrumpe la operación en curso.

### Pruebas ejecutadas

```powershell
cargo fmt --manifest-path apps\desktop\src-tauri\Cargo.toml
cargo test --manifest-path apps\desktop\src-tauri\Cargo.toml
cargo clippy --manifest-path apps\desktop\src-tauri\Cargo.toml --all-targets -- -D warnings
.\node_modules\.bin\tsc.CMD -b --pretty false
.\node_modules\.bin\vitest.CMD run
python -m unittest discover -s tests
git diff --check
```

Resultado: 94 pruebas Rust (87 previas + 7 nuevas), 58 de TypeScript y 17 de
Python en verde; clippy sin avisos.

Las siete pruebas nuevas de `logging.rs` cubren:

| Prueba | Qué demuestra |
|---|---|
| `secrets_paths_and_free_text_can_never_reach_the_log` | un token, una ruta con nombre de usuario, un prompt y un mensaje de error pasados por descuido salen los cuatro como `[redactado]`, mientras `status`, `attempt` y `error_kind` se conservan |
| `every_line_is_valid_json_with_utc_timestamp_and_version` | cada línea es JSON válido con `ts`, `event` y `app_version` |
| `timestamps_cover_leap_years_and_epoch_boundaries` | la conversión civil acierta en epoch, 29-feb-2024 y 1-mar-2000 |
| `identifiers_survive_but_arbitrary_text_does_not` | los UUID y claves cortas sobreviven; el texto con espacios, mayúsculas o exceso de longitud no |
| `the_log_rotates_by_size_and_keeps_a_single_previous_copy` | la rotación conserva exactamente un archivo previo |
| `error_classes_are_recorded_without_their_message` | de un `BrokerResponse` queda la clase, no el mensaje |
| `a_ready_sink_writes_one_json_line_per_event` | el destino real escribe una línea por evento |

### Limitaciones conocidas

- Un secreto **corto** (menos de 32 caracteres, en minúsculas y sin espacios)
  pasado deliberadamente como `Code` sí cabría en el formato. La defensa real es
  que ningún punto de instrumentación recibe el token: el cliente HTTP lo guarda
  como cabecera y nunca lo pasa al registro.
- El registro todavía no se purga por antigüedad, solo por tamaño.
- No hay aún visor del registro dentro de la aplicación; se expone su ruta.

### Verificación manual pendiente

- Abrir la aplicación con el BAT, provocar un fallo de red del Broker y revisar
  que `chatygpt.log` contiene `broker.transport_failed` y ningún dato personal.
- Comprobar que la información emergente del pie muestra la ruta del registro.

## 2. Expediente durable de confirmaciones

Estado: **implementado y verificado automáticamente**.

### Problema

`confirmation_requests` existía desde la migración inicial pero **ningún código
la escribía**: las confirmaciones vivían solo en el estado de React. Se pedía
permiso, sí, pero después no era posible demostrar qué se autorizó, cuándo ni
con qué información delante. Para un requisito que el propio encargo marca como
invalidante —«ejecución de acciones sensibles sin confirmación»— la prueba
importa tanto como el diálogo.

### Solución

La migración `0016_confirmation_requests.sql` vincula la tabla con la llamada de
herramienta y la conversación, con un índice único que impide dos expedientes
para la misma llamada. El ciclo queda así:

1. Cuando el Broker deja la tarea en `waiting_for_tools`, junto a cada
   `tool_call` nace un `confirmation_request` en estado `pending`, con acción,
   herramienta, recursos afectados, datos que se enviarán, destino, alcance
   temporal y consecuencias. Nace **antes** de que nadie pueda decidir, así que
   queda constancia de la propuesta aunque se cierre la aplicación sin responder.
2. `resolve_tool_calls` rechaza la petición si el expediente ya no está
   `pending`, **antes** de ejecutar nada.
3. `prepare_tool_outcomes` resuelve el expediente como `allowed_once` o
   `cancelled`, con fecha, en la misma transacción que escribe el resultado de la
   herramienta, y audita la decisión como `confirmation.resolved`.

Una herramienta que la aplicación no reconoce no recibe una descripción genérica
tranquilizadora: se declara destino «no declarado» y se recomienda rechazarla.

### Lo que ve la persona

La tarjeta de confirmación mostraba `JSON.stringify(call.arguments)`. Ahora
presenta los siete elementos exigidos —acción, herramienta, recursos, datos,
destino, alcance y consecuencias— en texto legible, y el botón dice **Autorizar
una vez**, que es exactamente el alcance que se concede.

### Pruebas ejecutadas

Mismos comandos que en el punto 1. Resultado: 95 pruebas Rust, 60 de TypeScript
y 17 de Python en verde; clippy sin avisos.

| Prueba | Qué demuestra |
|---|---|
| `tool_confirmation_is_disclosed_persisted_and_cannot_be_replayed` (Rust) | el expediente nace `pending` con los siete elementos, sobrevive a reabrir la base, se resuelve como `allowed_once` con fecha, se audita una sola vez y un reenvío de la misma decisión termina en `Conflict` |
| `muestra los siete elementos del expediente sin JSON técnico` (TS) | la proyección de interfaz no expone JSON crudo |
| `no tranquiliza cuando falta el expediente` (TS) | sin expediente, la tarjeta declara destino y recursos «no declarados» en lugar de inventarlos |

### Limitaciones conocidas

- La ejecución de la herramienta y la resolución del expediente ocurren en dos
  transacciones distintas. La comprobación previa impide la doble ejecución en el
  caso realista (reenvío de la decisión), pero una caída **entre** ambas dejaría
  la acción hecha con el expediente todavía `pending`. Unificarlo exige mover la
  ejecución dentro de la transacción de resultados.
- El estado `expired` previsto en el esquema todavía no se aplica: no hay
  caducidad automática de una confirmación no respondida.
- Las confirmaciones de ciclo de vida de la interfaz (borrar conversación,
  activar una tarea programada, sobrescribir una exportación) siguen auditándose
  como `audit_events` y aún no generan expediente.

### Verificación manual pendiente

- Habilitar **Renombrar conversación** en un GPT personal, pedir un título nuevo
  y comprobar que la tarjeta muestra los siete elementos antes de decidir.
- Autorizar una vez y comprobar en **Actividad reciente** que aparece la
  resolución.

## 3. Carpetas autorizadas para escritura

Estado: **implementado y verificado automáticamente**.

### Problema

`authorized_folders` tampoco tenía código. En la práctica la única barrera era
que cada exportación pasara por un selector nativo, lo cual protege mientras
todas las escrituras nazcan de un clic humano, pero no deja rastro revisable ni
resiste que un camino de código futuro construya la ruta por su cuenta.

### Solución

- Elegir un destino en el selector nativo **es** la concesión: los selectores de
  Markdown, historial `.txt`, calendario `.ics`, exportación de GPT y bóveda de
  Obsidian registran la carpeta contenedora con su propósito.
- `export.rs` valida el destino contra la tabla antes de escribir, en las cinco
  rutas de exportación. La comprobación acepta descendientes —la proyección de
  Obsidian crea subcarpetas dentro de la bóveda— pero nunca una carpeta hermana
  con nombre parecido.
- **Inicio → Carpetas autorizadas** lista las concesiones con su uso legible y
  permite revocarlas. Revocar cierra la puerta a futuras escrituras sin tocar lo
  ya exportado; volver a elegir la carpeta en el selector la reactiva.
- Conceder y revocar quedan auditados (`authorized_folder.granted` y
  `authorized_folder.revoked`) sin registrar la ruta en el evento.

La comparación de rutas canonicaliza cuando existen, retira el prefijo extendido
`\\?\` de Windows y normaliza mayúsculas, porque NTFS no las distingue.

### Pruebas ejecutadas

Mismos comandos que en los puntos anteriores. Resultado: 96 pruebas Rust, 62 de
TypeScript y 17 de Python en verde; clippy sin avisos.

| Prueba | Qué demuestra |
|---|---|
| `writing_outside_an_authorized_folder_is_refused_until_it_is_granted` (Rust) | sin concesión la exportación termina en `Conflict` y **no crea el archivo**; con la carpeta concedida funciona; tras revocarla vuelve a rechazarse y lo ya exportado permanece intacto |
| `traduce el uso concedido a lenguaje comprensible` (TS) | la lista muestra el propósito real de cada concesión |
| `no inventa un uso cuando la concesión no lo declara` (TS) | una concesión sin propósito se muestra como «Uso no declarado» |

Los cuatro tests de exportación existentes empezaron a fallar al introducir la
comprobación —la prueba de que la restricción muerde— y ahora conceden su carpeta
temporal igual que haría una persona en el selector.

### Limitaciones conocidas

- La copia administrada de adjuntos y la base SQLite viven en `AppLocalData` y no
  pasan por esta comprobación: son almacenamiento propio de la aplicación, no
  escrituras en carpetas del usuario.
- Los permisos guardados son `{"write": true}`; todavía no se distingue lectura
  de escritura porque ninguna función necesita hoy leer carpetas arbitrarias.
- El selector de importación (leer un GPT exportado) no concede nada, que es lo
  correcto, pero tampoco deja constancia de la lectura.

### Verificación manual pendiente

- Exportar una conversación, abrir **Inicio → Carpetas autorizadas**, revocar la
  carpeta y comprobar que la siguiente exportación al mismo destino se rechaza
  con un mensaje accionable.

## 4. Custodia del token de Broker AI

Estado: **implementado; ciclo criptográfico verificado en el equipo real**.

### Decisión

El prompt admite «el almacén seguro de credenciales del sistema operativo o un
mecanismo equivalente». Se ha elegido **DPAPI en ámbito `CurrentUser`** en lugar
de Credential Manager por tres razones: el proyecto ya lo usaba para el inicio
con Windows, no añade dependencias nativas nuevas y ata el secreto a la cuenta
de Windows —ni otra cuenta del equipo ni una copia del archivo en otra máquina
pueden descifrarlo—. El archivo vive en `<AppLocalData>/credentials/`, fuera de
cualquier carpeta sincronizada.

### Cambios

- `secrets.rs` centraliza guardar, cargar, retirar y diagnosticar la credencial.
  Valida el token antes de cifrarlo (no vacío, sin caracteres de control, hasta
  512 caracteres) y, tras cifrar, **verifica que el archivo resultante no
  contiene el secreto en claro**; si lo contuviera, lo borra en lugar de dejarlo.
- `BrokerClient` guarda el token en `Arc<RwLock<…>>`: rotarlo se aplica en
  caliente, sin reiniciar la aplicación.
- La resolución es almacén protegido → variable de entorno. La variable queda
  degradada a vía de transición y ya no es la fuente principal.
- **Inicio → Credencial de Broker AI** permite guardarla y retirarla, muestra su
  origen real y nunca la devuelve al frontend.
- Guardar y retirar quedan auditados como `broker_credential.changed`, con la
  clase de protección pero sin el valor. El registro estructurado solo anota
  `broker.credential_stored` / `broker.credential_cleared`, sin campos.
- `startup.rs` deja de duplicar la lógica DPAPI y toma el token del almacén, de
  modo que activar el inicio con Windows ya no exige abrir la app con el BAT.
- `Arrancar ChatyGPT.bat` reutiliza la credencial guardada y solo pregunta si no
  existe o no puede descifrarse.

### Defecto encontrado y corregido

Al ensayar el cifrado se descubrió que **Windows PowerShell 5.1 no carga
`System.Security` por defecto**, por lo que `[Security.Cryptography.ProtectedData]`
no existía y el cifrado fallaba. Afectaba al código de inicio con Windows ya
existente, cuya activación manual seguía pendiente en la Fase 4. Se añadió
`Add-Type -AssemblyName System.Security` en los tres puntos que lo usan (guardar,
cargar y el script de arranque) y una aserción en la prueba del script.

Comprobación real ejecutada en este equipo:

```
bytes cifrados: 262
contiene el token en claro: False
descifrado correcto: True
```

### Pruebas ejecutadas

Mismos comandos que en los puntos anteriores. Resultado: 99 pruebas Rust, 63 de
TypeScript y 17 de Python en verde; clippy sin avisos.

| Prueba | Qué demuestra |
|---|---|
| `an_unencrypted_blob_is_never_accepted_as_protected` (Rust) | un blob que contenga el token, esté vacío o sea sospechosamente corto se rechaza; solo se acepta uno con forma de salida DPAPI |
| `stored_tokens_reject_empty_control_and_oversized_values` (Rust) | un token vacío, con saltos de línea o excesivo se rechaza y **no crea el almacén** |
| `credential_status_prefers_the_protected_store_over_the_environment` (Rust) | el estado distingue almacén, entorno y ausencia, y retirar dos veces no es un error |
| `startup_script_waits_for_authenticated_broker_without_embedding_the_token` (Rust) | el script de arranque carga `System.Security`, descifra y no incrusta el token |
| `nombra el origen real de la credencial en uso` (TS) | la interfaz no dice «guardada» cuando en realidad viene del entorno |

### Limitaciones conocidas

- DPAPI se invoca a través de PowerShell, no de la API nativa: el token viaja por
  el entorno del proceso hijo (nunca por la línea de órdenes, que sí es visible
  en la lista de procesos) y vuelve por su salida estándar al descifrar.
  Sustituirlo por `CredWriteW`/`CredReadW` exigiría una dependencia nativa.
- No hay bloqueo por contraseña maestra: quien tenga la sesión de Windows abierta
  puede usar la credencial, igual que ocurre con Credential Manager.
- El token sigue admitiéndose desde el entorno; retirarlo del todo obligaría a
  reescribir el BAT y el flujo de arranque con Windows.

### Verificación manual pendiente

- Guardar el token real en **Inicio → Credencial de Broker AI**, cerrar la
  aplicación, abrirla sin variable de entorno y comprobar que **Comprobar
  conexión** sigue funcionando.
- Activar **Inicio con Windows** sin variable de entorno y confirmar que ya no
  exige abrir la aplicación con el BAT.

## 5. Cobertura medida e integración continua

Estado: **cobertura de Rust medida; CI escrita y pendiente de su primera
ejecución en GitHub**.

### Cobertura real de Rust

Medida con `cargo-llvm-cov` sobre la biblioteca, el 1 de agosto de 2026:

```powershell
cargo llvm-cov --manifest-path apps\desktop\src-tauri\Cargo.toml --lib --summary-only
```

| Módulo | Líneas cubiertas | Lectura |
|---|---|---|
| `db/mod.rs` | **86,30 %** | dominio y persistencia, por encima del 80 % exigido |
| `logging.rs` | **84,06 %** | observabilidad y redacción |
| `export.rs` | **81,53 %** | exportación, conflictos y carpetas autorizadas |
| `task_runtime.rs` | **58,29 %** | **por debajo** del 80 % que el encargo pide para polling y recuperación |
| `secrets.rs` | **53,23 %** | la parte no cubierta es la que invoca DPAPI real |
| `startup.rs` | 29,17 % | casi todo son llamadas al registro de Windows |
| `broker/mod.rs` | 5,08 % | necesita un servidor HTTP simulado |
| `lib.rs` | 2,53 % | comandos Tauri; solo se ejercitan con la aplicación en marcha |
| **Total** | **71,21 %** | cumple el 70 % global de lógica no visual, con poco margen |

La cifra global cumple el umbral del encargo, pero **dos objetivos concretos
todavía no se alcanzan** y conviene decirlo con claridad:

- polling y recuperación (`task_runtime.rs`) están en 58 %, no en 80 %;
- el adaptador de Broker AI apenas se ejercita porque no hay servidor simulado.

> **Actualización del 4 de agosto de 2026.** Ambos objetivos quedan alcanzados
> con el Broker simulado: `task_runtime.rs` sube a ~81,5 %, `broker/mod.rs` a
> ~87,8 %, `attachment_runtime.rs` a ~82,6 % y el total a ~78,8 %. Ver el
> punto 6.

### Integración continua

`.github/workflows/ci.yml` ejecuta en `windows-latest` —la plataforma real de la
aplicación— `cargo fmt --check`, `cargo clippy -D warnings`, las pruebas de Rust
con cobertura y umbral (`--fail-under-lines 70`), `pnpm typecheck`,
`pnpm test:coverage`, `pnpm build`, las pruebas de Python y `git diff --check`.
Un segundo trabajo repite en Linux solo lo independiente del sistema.

El umbral no es decorativo: con 71,21 % actual, cualquier bloque nuevo sin
pruebas hace fallar la CI.

### Cobertura de TypeScript

`vite.config.ts` declara el proveedor `v8`, un umbral del 70 % y excluye `App.tsx`
y `platform.ts`, que son capa de presentación y de transporte sin pruebas de
componente todavía; incluirlos daría una cifra engañosa.

**No se ha podido ejecutar en este equipo**: falta `@vitest/coverage-v8` y
`pnpm install` exige purgar `node_modules`, una operación destructiva que no se
ha hecho sin permiso. La dependencia queda declarada en `package.json` y la CI la
instalará limpiamente. Hasta esa primera ejecución, la cobertura de TypeScript
es **desconocida**, no «cumplida».

> **Actualización del 5 de agosto de 2026.** Ya se ejecuta, y resultó que la
> configuración no medía ningún archivo: el umbral se cumplía sobre `0/0`. Con
> las rutas corregidas la cobertura real es del 81,3 % de líneas y el umbral
> sube a 78. Ver el punto 8.

### Pruebas ejecutadas

```powershell
cargo fmt --manifest-path apps\desktop\src-tauri\Cargo.toml
cargo test --manifest-path apps\desktop\src-tauri\Cargo.toml
cargo clippy --manifest-path apps\desktop\src-tauri\Cargo.toml --all-targets -- -D warnings
cargo llvm-cov --manifest-path apps\desktop\src-tauri\Cargo.toml --lib --summary-only
.\node_modules\.bin\tsc.CMD -b --pretty false
.\node_modules\.bin\vitest.CMD run
python -m unittest discover -s tests
git diff --check
```

Resultado: 99 pruebas de Rust, 63 de TypeScript y 17 de Python en verde; clippy
sin avisos; cobertura de Rust 71,21 % de líneas.

### Limitaciones conocidas

- La CI nunca se ha ejecutado: el flujo se ha escrito, no comprobado. Su primera
  ejecución puede necesitar ajustes de instalación de dependencias.
- No hay pruebas end-to-end ni de interfaz; siguen siendo la mayor deuda de
  calidad del proyecto junto con la cobertura de `task_runtime.rs`.
- `pnpm install` está roto en este equipo: pnpm quiere purgar `node_modules` y
  no puede confirmarlo sin terminal interactiva. Las herramientas se ejecutan
  desde `node_modules\.bin` mientras tanto.

### Trabajo pendiente inmediato

1. ~~Subir `task_runtime.rs` al 80 %~~ y ~~añadir un servidor HTTP simulado~~:
   ambos cerrados el 4 de agosto de 2026, ver el punto 6.
2. Ejecutar la CI y ajustar lo que falle.

## 6. Broker AI simulado y cobertura de polling y recuperación

Estado: **implementado y verificado automáticamente**, el 4 de agosto de 2026.

### Problema

Los dos huecos que quedaban eran en realidad uno. Envío, reintento, sondeo,
espera de herramientas, recuperación, ingesta de adjuntos y diagnóstico no son
funciones puras: son intercambios HTTP y bucles asíncronos que reaccionan a
códigos de estado. Ninguna prueba sobre construcción de peticiones demuestra que
**terminan donde deben**, y por eso `task_runtime.rs` se quedaba en 58 %,
`attachment_runtime.rs` en 44 % y `broker/mod.rs` en 5 %.

Faltaba, además, poder provocar a voluntad lo que un Broker real no ofrece:
un 503 justo en el primer envío, un 422 al sondear, o una tarea que se completa
exactamente porque recibió la decisión sobre una herramienta.

### Solución

`broker/simulated.rs` (solo bajo `cfg(test)`) levanta un servidor HTTP real en
`127.0.0.1` con puerto efímero. **No añade ninguna dependencia**: habla HTTP/1.1
con `Connection: close`, que es todo lo que el cliente necesita. Aporta tres
cosas que las pruebas necesitan:

- **respuestas programadas** por ruta, en secuencia y con respuesta de reserva;
- **transiciones causales** (`after`): recibir los resultados de una herramienta
  es lo que completa la tarea, no el paso del tiempo. Sin esto, una prueba
  dependería de qué llamada llega antes y sería intermitente;
- un **registro de peticiones** con el que comprobar que un reintento reutiliza
  la clave idempotente y que recuperar una tarea no la reenvía.

`BrokerClient::for_base_url` —también `cfg(test)`— apunta el cliente al
simulador sin tocar el almacén DPAPI ni la variable de entorno, de modo que las
pruebas no dependen de la configuración del equipo.

### Defecto encontrado y corregido

La primera versión fallaba de forma intermitente bajo carga. La causa no era el
código de producción sino el simulador: el listener acepta sin bloquear para
poder apagarse, y en Windows **el socket aceptado hereda ese modo**. Cuando los
bytes de la petición aún no habían llegado, la lectura devolvía `WouldBlock` y
la conexión se cerraba sin responder, lo que el cliente veía como un fallo de
red. Se corrige con `set_nonblocking(false)` sobre el socket aceptado. La suite
se ejecutó tres veces seguidas en verde para confirmarlo.

### Pruebas ejecutadas

```powershell
cargo fmt --check --manifest-path apps\desktop\src-tauri\Cargo.toml
cargo test --manifest-path apps\desktop\src-tauri\Cargo.toml
cargo clippy --manifest-path apps\desktop\src-tauri\Cargo.toml --all-targets -- -D warnings
cargo llvm-cov --manifest-path apps\desktop\src-tauri\Cargo.toml --lib --summary-only
```

Resultado: 135 pruebas de Rust en verde, clippy sin avisos, tres pasadas
consecutivas de la suite sin intermitencias.

| Prueba | Criterio de aceptación que demuestra |
|---|---|
| `chat_turn_polls_until_terminal_and_materializes_the_answer` | «polling aplica límites y termina en estados terminales»: pasa por una fase intermedia, para en el estado terminal y **deja de sondear**, y materializa la respuesta |
| `transient_failure_is_retried_with_the_same_idempotency_key` | «la misma operación reintentada no duplica la tarea»: dos envíos, una sola clave idempotente y un único identificador remoto |
| `permanent_rejection_orphans_the_task_without_retrying` | un 422 al enviar deja la tarea huérfana y **no** entra en el bucle de reintento |
| `transient_polling_errors_are_retried_without_losing_the_task` | distingue «el Broker no responde ahora» de «esta tarea no existe»: conserva la identidad remota y no reenvía |
| `permanent_polling_error_orphans_the_task_instead_of_looping` | un error de contrato al sondear detiene el bucle en lugar de repetirlo indefinidamente |
| `polling_waits_for_a_tool_decision_and_resumes_after_it` | ninguna herramienta se ejecuta sin confirmación: sin decisión no se envía nada, y tras decidir se envía **una sola vez** |
| `remote_failure_is_reported_instead_of_being_answered` | un fallo remoto deja el mensaje fallido con su error, sin fabricar contenido |
| `restart_resumes_an_active_task_without_submitting_it_again` | «un reinicio recupera tareas activas sin pérdida»: se sondea, no se reenvía |
| `cancellation_reflects_the_real_broker_response` | la cancelación persiste lo que devuelve el Broker, no una suposición local |

Sobre el propio adaptador (`broker/mod.rs`):

| Prueba | Qué demuestra |
|---|---|
| `diagnosis_separates_unreachable_not_ready_and_ready` | los tres estados con los que la interfaz decide si deja enviar no se confunden; un Broker sano con capacidades ilegibles se declara **accesible y no listo**, nunca listo |
| `responses_are_classified_as_contract_or_response_errors` | un HTTP 200 con cuerpo ilegible es fallo de contrato, no de red; un 422 conserva código y detalle publicado |
| `the_admin_token_travels_only_after_it_is_configured` | la credencial viaja en `x-admin-token`, se rota y se retira en caliente, y un token con caracteres imposibles se rechaza **sin citar su contenido** en el error |
| `file_upload_sends_the_real_content_and_reads_its_state` | el multipart lleva nombre, tipo y contenido reales del archivo, y el estado posterior se lee del Broker |
| `a_rejected_upload_keeps_the_broker_status` | una subida rechazada no se disfraza de éxito |
| `converted_markdown_download_is_bounded_to_http_and_utf8` | acepta ruta relativa y URL absoluta; rechaza esquemas no web —`file://` incluido—, errores del servidor y cuerpos no UTF-8 en lugar de guardarlos con pérdidas |
| `only_web_schemes_are_accepted_as_base_url` | la base se valida y se normaliza para que `join` no pierda la ruta |

Sobre la ingesta de adjuntos (`attachment_runtime.rs`):

| Prueba | Qué demuestra |
|---|---|
| `ingestion_uploads_polls_and_stores_the_converted_context` | el recorrido completo —copia gestionada, subida, sondeo, conversión y fragmentación—; el archivo se sube **una sola vez** y con su contenido real |
| `a_fingerprint_mismatch_fails_the_attachment_instead_of_trusting_the_broker` | si la huella devuelta no es la del archivo local, el adjunto falla y **no adopta el archivo ajeno**: garantiza que el contexto enviado al modelo procede del documento que la persona adjuntó |
| `a_failed_conversion_download_degrades_context_without_losing_the_attachment` | perder la conversión degrada el contexto, no el adjunto: `ingestion_status` sigue `ready` y el error se registra en `context_status` |
| `an_attachment_without_conversion_declares_its_context_unavailable` | sin Markdown publicado el contexto se declara **no disponible**, que es una ausencia, no un error |
| `retry_and_restart_resume_a_failed_ingestion` | reintentar limpia el error y vuelve a subir; recuperar al arrancar **no vuelve a subir** lo ya ingerido |

### Cobertura resultante

| Módulo | Antes | Ahora |
|---|---|---|
| `task_runtime.rs` | 58,29 % | **~81,5 %** — cumple el 80 % que el encargo pide para polling y recuperación |
| `broker/mod.rs` | 5,08 % | **~87,8 %** |
| `attachment_runtime.rs` | 44,07 % | **~82,6 %** |
| `broker/simulated.rs` | — | ~97 % (el propio simulador) |
| **Total** | 71,21 % | **~78,8 %** |

Las cifras se dan aproximadas a propósito: oscilan una décima entre pasadas
porque los bucles de sondeo ejecutan un número distinto de vueltas según los
tiempos de la máquina. Por eso el umbral de la CI sube de 70 a **77** y no a 78:
debe fallar ante una regresión real, no ante esa oscilación.

### Limitaciones conocidas

- El simulador es fiel al **contrato**, no al comportamiento: no encola, no
  enruta ni aplica presupuestos. Las pruebas de integración real contra un
  Broker en marcha siguen siendo necesarias y siguen siendo manuales.
- Lo que queda sin cubrir en `broker/mod.rs` y `attachment_runtime.rs` son
  ramas de error de transporte que exigirían cortar la conexión a mitad de
  respuesta, y el backoff de reintento de subida, cuya primera espera es de dos
  segundos y encarecería la suite sin demostrar nada nuevo.
- Los módulos que siguen bajos son `lib.rs` (comandos Tauri, solo ejercitables
  con la aplicación en marcha), `startup.rs` y `secrets.rs` (registro de Windows
  y DPAPI reales) y `scheduler_runtime.rs`. Ninguno es lógica de dominio.
- Sigue sin haber pruebas end-to-end ni de interfaz: es ya la mayor deuda de
  calidad que queda. El punto 7 cubre parte del hueco de forma estática.

## 7. Contrato de la interfaz y confirmaciones realmente obtenidas

Estado: **implementado, con cinco defectos encontrados y corregidos**, el 5 de
agosto de 2026.

### Problema

Las órdenes de Tauri se enlazan **por cadena de texto**. El frontend escribe
`invoke("delete_memory_item", { memoryId })` y Rust declara
`fn delete_memory_item(memory_id: String, ...)`. No hay tipos compartidos: un
nombre mal escrito, una orden que se olvida de registrar en `generate_handler!`
o un argumento renombrado en un solo lado **compilan sin protestar** y fallan
solo al ejecutar la aplicación y pulsar ese botón concreto. Sin pruebas
end-to-end, ese fallo llega hasta el usuario.

El segundo problema es más serio. Varias órdenes de Rust exigen
`confirmed = true` antes de actuar, pero es `platform.ts` quien fija ese valor.
Si quien llama no pregunta antes, la comprobación no protege nada: es una
confirmación **afirmada**, no obtenida. El encargo trata la ejecución de
acciones sensibles sin confirmación como defecto invalidante.

### Solución

`tests/test_frontend_contract.py` lee `lib.rs`, `platform.ts` y `App.tsx` y
comprueba cinco propiedades del contrato —toda orden declarada está registrada,
toda invocada existe, los argumentos coinciden tras la conversión a
`snake_case` que aplica Tauri, y ninguna orden queda inalcanzable— más la
propiedad de confirmación: **toda orden que recibe `confirmed` debe llamarse
desde una función que antes haya preguntado**. La comprobación analiza el
cuerpo equilibrado de la función que contiene la llamada, no una ventana de
caracteres alrededor, para no dar por buena una confirmación que pertenece a
otra función vecina.

El contrato resultó estar limpio: 102 órdenes, todas declaradas, registradas,
invocadas y con argumentos coincidentes.

### Defectos encontrados y corregidos

La prueba de confirmación sí encontró cinco rutas que enviaban `confirmed: true`
sin haber preguntado nunca:

| Acción | Por qué importa |
|---|---|
| Retirar la credencial de Broker AI | deja el equipo sin poder enviar mensajes hasta volver a introducirla |
| Revocar una carpeta autorizada para escritura | es una decisión de permisos, justo lo que el punto 3 exige confirmar |
| Reactivar una tarea programada | devuelve a la tarea la capacidad de lanzar trabajos contra el Broker **sin nadie delante**; Rust lo exige solo al reactivar, no al pausar, y ahora se pregunta en ese mismo caso |
| Restaurar una versión anterior de un GPT | reemplaza la configuración vigente |
| Vaciar las mediciones de rendimiento | es irreversible y deja las cuatro métricas sin veredicto |

Las cinco preguntan ahora antes de llamar. Ninguna era un fallo de Rust: la
comprobación existía y era correcta: lo que faltaba era la pregunta que debía
respaldarla.

### Pruebas ejecutadas

```powershell
python -m unittest discover -s tests
.\node_modules\.bin\tsc.CMD -b --pretty false
.\node_modules\.bin\vitest.CMD run
```

Resultado: 23 pruebas de Python en verde (6 nuevas), TypeScript sin errores de
tipos y 74 pruebas de TypeScript en verde.

### Limitaciones conocidas

- Desde el 6-ago-2026 hay una comprobación equivalente hacia el otro lado:
  `tests/test_broker_task_state_contract.py` valida contra
  `contracts/broker/2.7/task-state.response.json` —copia literal del esquema que
  publica AI Broker— las respuestas de tarea que ChatyGPT da por buenas. Si una
  de esas formas dejara de cumplir el contrato, las pruebas se estarían
  apoyando en algo que el Broker no promete y pasarían en verde mientras la
  aplicación falla contra el Broker real. El esquema no se edita aquí: si el
  Broker cambia, se vuelve a copiar y la prueba dice qué se rompe. Requiere
  `jsonschema`, declarado en `tests/requirements.txt`.
- Es un análisis **estático** del código fuente, no una prueba end-to-end: no
  demuestra que el botón exista, esté visible ni sea pulsable. Demuestra que el
  cableado es coherente y que la confirmación está escrita.
- La detección de confirmaciones reconoce `window.confirm(...)`, el sistema de
  diálogos y las casillas de estado del formulario. Una confirmación
  implementada de otra forma daría un falso positivo y habría que enseñársela.
- La detección de confirmaciones es estática; el punto 8 la comprueba además
  ejecutando la interfaz.

## 8. Pruebas de interfaz y cobertura real de TypeScript

Estado: **implementado, con dos defectos encontrados y corregidos**, el 5 de
agosto de 2026.

`pnpm` volvió a funcionar en el equipo, lo que permitió instalar
`@testing-library/react`, `@testing-library/user-event` y `jsdom` y montar la
aplicación de verdad en las pruebas. Se comprobó antes que
`pnpm install --frozen-lockfile` reproduce el árbol sin romper nada.

### Primer defecto: dos paneles de seguridad no se cargaban al arrancar

La credencial de Broker AI y las carpetas autorizadas se pedían **solo** desde
`reloadNavigation`, que se ejecuta después de una acción de la persona —enviar
un mensaje, crear o borrar algo—. Al abrir ChatyGPT y no hacer nada, ambos
paneles se quedaban indefinidamente en «Comprobando credencial…» y «Cargando
permisos…».

Quien abriera la aplicación solo para revisar su credencial o revocar una
carpeta —es decir, exactamente el uso que motivó ambos paneles en los puntos 3
y 4— no llegaba a verlas nunca. Ninguna prueba anterior podía detectarlo: el
backend respondía bien, el contrato era correcto y el componente compilaba. Hizo
falta montar la interfaz y mirar qué había en pantalla.

Ambas cargas se hacen ahora en el arranque, junto al resto del estado inicial.

### Segundo defecto: el umbral de cobertura de TypeScript nunca midió nada

`vite.config.ts` declara `root: "apps/desktop"`, pero el patrón de cobertura
estaba escrito desde la raíz del repositorio
(`apps/desktop/src/**/*.{ts,tsx}`). Resuelto contra `root`, apuntaba a
`apps/desktop/apps/desktop/src/…`, que no existe. El resultado era
`0/0` y un `Unknown%` que **satisfacía el umbral del 70 % sin medir un solo
archivo**. La CI lleva ejecutando esa comprobación desde que se escribió y
siempre la ha pasado en vacío.

Corregidas las rutas, la cobertura real es del **81,3 % de líneas**, 83,6 % de
ramas y 90,1 % de funciones. El umbral sube de 70 a **78**, por el mismo
criterio que en Rust: debe seguir a lo medido para detectar regresiones.

`platform.ts` queda fuera del umbral —no de la compilación— porque no contiene
decisiones: son envoltorios mecánicos de `invoke`, y su corrección real la
comprueba `tests/test_frontend_contract.py` contra el código de Rust, que es una
garantía más fuerte que una prueba afirmando el literal recién escrito.

### Pruebas ejecutadas

```powershell
pnpm install --frozen-lockfile
pnpm test
pnpm test:coverage
pnpm typecheck
pnpm build
python -m unittest discover -s tests
cargo test --manifest-path apps\desktop\src-tauri\Cargo.toml
```

Resultado: 79 pruebas de TypeScript en verde (5 nuevas de interfaz), 23 de
Python, 135 de Rust, tipos y compilación limpios.

| Prueba | Qué demuestra |
|---|---|
| `carga credencial y carpetas autorizadas sin necesidad de actuar antes` | los dos paneles están **visibles** al abrir, no solo pedidos |
| `no retira la credencial si la persona cancela la confirmación` | cancelar no llama a Rust |
| `retira la credencial solo después de aceptar` | aceptar sí lo hace, y la pregunta **precede** a la llamada |
| `no revoca una carpeta autorizada si la persona cancela` | igual para los permisos de escritura |
| `no vacía las mediciones de rendimiento si la persona cancela` | igual para las mediciones |

### Limitaciones conocidas

- `App.tsx` queda fuera del umbral de cobertura: son 7.000 líneas de
  presentación con cinco pruebas de componente. Incluirlo hundiría la cifra de
  la lógica que sí está cubierta y ocultaría una regresión real en ella. Las
  pruebas existen y se ejecutan; lo que no se hace es fingir que lo cubren. El
  punto 9 ataca la causa en lugar del síntoma.
- Siguen sin existir pruebas end-to-end sobre la aplicación empaquetada: estas
  montan React con `jsdom` y un doble de `platform`, no ejecutan Tauri ni
  WebView2.

## 9. Reducción de `App.tsx` por fases

Estado: **las cuatro fases completadas**, el 5 de agosto de 2026.

### Por qué

Excluir `App.tsx` del umbral de cobertura es honesto pero no resuelve nada: la
lógica que contiene sigue sin comprobarse. La causa no es que sea presentación,
sino que **mezcla decisiones con JSX** dentro de un único componente de 7.000
líneas, donde nada puede ejercitarse por separado.

El plan es sacar esas decisiones a módulos propios, como ya ocurre con
`domain.ts`, y hacerlo **por fases con sus pruebas antes de pasar a la
siguiente**, para que un fallo se localice en el cambio que lo introdujo y no en
una refactorización de miles de líneas.

### Fase 1: ayudantes de módulo

Las funciones que ya vivían fuera del componente, pero en el mismo archivo y sin
una sola prueba. Salen tal cual, sin cambiar comportamiento:

| Módulo nuevo | Qué contiene | Por qué merece pruebas |
|---|---|---|
| `dialogs.ts` | `DialogState` y `dialogCopy` | decide si una acción se anuncia como **destructiva**, qué se promete que ocurrirá y con qué palabra se confirma |
| `schedulerView.ts` | hora local, etiquetas de ejecución y avisos leídos | la hora se formatea en hora de pared, no en UTC; los avisos leídos se acotan y degradan sin romper si `localStorage` falla |
| `errors.ts` | `describeError` | es la única puerta por la que un fallo llega a la pantalla, y casi siempre recibe la cadena de `AppError`, no una instancia de `Error` |

`App.tsx` baja 138 líneas y la cobertura de TypeScript sube del 81,3 % al
**83,1 %** de líneas, con 19 pruebas nuevas.

### Fase 2: proyecciones derivadas del estado

Cálculos que vivían dentro del componente y decidían qué ve la persona:

| Destino | Qué se extrae | Regla que ahora queda fijada |
|---|---|---|
| `domain.ts` | `visibleConversations` | **buscar tiene prioridad sobre el ámbito de proyecto**: los resultados se muestran completos aunque haya un proyecto seleccionado, porque quien busca espera encontrar y no que el filtro activo le esconda lo que buscaba |
| `domain.ts` | `memoryAppliesToConversation`, `activeMemoriesForConversation`, `semanticReadyMemoriesForConversation` | un recuerdo sin proyecto es global; uno acotado no se filtra a otro proyecto ni a una conversación sin proyecto. Los indexados son por construcción un **subconjunto** de los activos |
| `schedulerView.ts` | `schedulerCalendarDays`, `schedulerCalendarConflictCount` | las atrasadas van a una cesta propia y no a la fecha que les tocaba; los conflictos se cuentan por pareja, no por mención, porque las dos automatizaciones implicadas declaran el mismo |

**Defecto de diseño corregido de paso:** la regla de alcance de un recuerdo
estaba escrita **dos veces** en `App.tsx` —una para contar los activos y otra
para contar los que tienen índice—. Esa duplicación es exactamente la forma en
que estas reglas se desincronizan: basta con cambiar una y olvidar la otra para
que la interfaz cuente recuerdos que no se van a usar. Ahora la segunda se
define como un filtro sobre la primera, así que no pueden discrepar.

`App.tsx` baja otras 25 líneas y la cobertura sube al **83,9 %**, con 14 pruebas
nuevas.

### Fase 3: decisiones previas a llamar al backend

Lo que la interfaz decide **antes** de que un mensaje o una automatización
lleguen a Rust. Estaba escrito como cadenas de `return` dentro de funciones con
estado y llamadas de red por medio, así que no había forma de comprobar ni el
orden ni los casos límite.

| Destino | Qué se extrae | Regla que ahora queda fijada |
|---|---|---|
| `composer.ts` | `sandboxDeniedByCustomGpt` | el permiso del GPT se comprueba **antes de tocar la red**: pedir el diagnóstico del Broker para después negarse por un permiso local sería una espera inútil |
| `composer.ts` | `sandboxSendDecision` | el orden completo: si el turno ya lleva Código aislado se envía sin volver a preguntar; si el mensaje pide ejecutar código y el sandbox está, se **propone**; si no está, se **rechaza** en lugar de enviar algo que no podrá ejecutarse; y tras responder a la propuesta no se vuelve a interrumpir |
| `schedulerView.ts` | `validateScheduleDraft` | distingue **incompleto** de **inválido**: un formulario a medias no es un error y no muestra nada, mientras que una fecha pasada sí se explica. La confirmación cuenta como dato obligatorio, que es lo que impide activar una automatización sin decidirlo |
| `schedulerView.ts` | `canSaveScheduleTemplate` | una plantilla solo necesita nombre e instrucción: no programa nada, así que no pide conversación, fecha ni confirmación |

La decisión del compositor pasa a ser un dato (`send`, `suggest-sandbox`,
`blocked`) que el componente se limita a obedecer. Eso permitió comprobar por
primera vez casos que antes solo podían revisarse leyendo: que un adjunto
tabular cambia el texto del rechazo, y que el segundo intento no vuelve a
bloquear aunque el sandbox siga sin estar disponible.

A diferencia de las fases anteriores, esta **no reduce** `App.tsx` —queda en
7.187 líneas, seis más—: los puntos de llamada son ahora más explícitos. El
beneficio no es el tamaño sino que las decisiones han salido del componente.
La cobertura sube al **84,6 %**, con 15 pruebas nuevas.

### Fase 4: máquinas de estado de la interfaz

| Destino | Qué se extrae | Regla que ahora queda fijada |
|---|---|---|
| `schedulerView.ts` | `pendingScheduledRunNotifications` | avisar **exactamente una vez** por finalización: solo transiciones, nunca en el primer sondeo, solo estados terminales, y sin permiso no se emite pero **sí se recuerda** |
| `domain.ts` | `shouldPollMemoryIndex`, `shouldPollMemorySearch` | condiciones de parada de los sondeos: equivocarse deja un temporizador vivo para siempre o corta la actualización antes de tiempo |
| `domain.ts` | `shouldReloadConversationAfterTurn` | recargar solo la conversación abierta: recargar otra sobrescribiría lo que la persona está leyendo |

La detección de avisos era la pieza más delicada de la interfaz, porque el error
se paga en las dos direcciones: avisar de más molesta cada diez segundos, y
avisar de menos deja pasar desapercibida una automatización que falló. Devolver
el estado siguiente en lugar de mutar un `ref` permitió comprobar la regla que
lo sostiene: **el estado se recuerda aunque no haya permiso**, de modo que
conceder el permiso más tarde no provoca una ráfaga de avisos atrasados sobre
ejecuciones ya vistas. También se fija que las ejecuciones ausentes de la
respuesta se conservan: las tarjetas solo traen los diez runs recientes, y
olvidar los anteriores los haría parecer nuevos al volver a aparecer.

`App.tsx` queda en 7.183 líneas y la cobertura sube al **85,1 %**, con 13
pruebas nuevas. El umbral pasa de 78 a **82**.

### Lo que la extracción no ha conseguido

El plan afirmaba que, con la lógica fuera, `App.tsx` podría entrar en el umbral
sin hundirlo. **Se ha medido y no es cierto todavía.** Incluyéndolo, la cobertura
global cae al **35,8 %**: `App.tsx` aporta unas 6.500 sentencias y las cinco
pruebas de interfaz solo ejercitan un 25 % de ellas al montar la aplicación.

La conclusión no es que la extracción sobrara —la cobertura de la lógica medida
subió del 81,3 % al 85,1 % y varias reglas quedaron fijadas por primera vez—
sino que **lo que queda en `App.tsx` es genuinamente presentación**: ramas de
JSX que solo se recorren renderizando cada panel, diálogo y estado. Cerrar ese
hueco pide más pruebas de interfaz, no más extracción. Seguir troceando el
componente daría módulos artificiales sin mejorar ninguna garantía.

### Pruebas ejecutadas

```powershell
pnpm typecheck
pnpm test
pnpm test:coverage
pnpm build
python -m unittest discover -s tests
```

Resultado tras la fase 4: 140 pruebas de TypeScript en verde (61 nuevas entre
las cuatro fases), 23 de Python, 135 de Rust, tipos y compilación limpios,
cobertura de TypeScript al 85,1 % con umbral 82.

### Limitaciones conocidas

- El siguiente paso para `App.tsx` no es más extracción sino **más pruebas de
  interfaz**, una por panel y estado. Es trabajo mecánico y largo; conviene
  hacerlo cuando el diseño esté estable, para no reescribir las pruebas con
  cada cambio visual.
- Las cuatro fases son movimientos **sin cambio de comportamiento**; las pruebas
  fijan lo que el código ya hacía, incluidos detalles discutibles como que
  `describeError(null)` muestre `"null"`. Documentarlo es preferible a
  cambiarlo de paso: un cambio de comportamiento debe ser una decisión
  explícita, no el efecto colateral de una refactorización. La única excepción
  es la deduplicación de la regla de alcance de los recuerdos, que unifica dos
  copias idénticas y por tanto tampoco cambia lo que hace hoy.
- `schedulerCalendarDays` depende de la zona horaria y del idioma del equipo
  (`toLocaleDateString("es-ES")`). Las pruebas construyen las fechas en hora
  local para no depender de la zona de la máquina que las ejecute, pero la
  etiqueta visible sí variaría en un sistema con otro idioma.
