# Endurecimiento de Fase 0

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

1. Subir `task_runtime.rs` al 80 % con pruebas de backoff, reintento,
   `waiting_for_tools` y recuperación tras reinicio.
2. Añadir un servidor HTTP simulado para el adaptador de Broker AI.
3. Ejecutar la CI y ajustar lo que falle.
