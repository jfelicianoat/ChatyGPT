# Evidencias de Fase 0

Este documento se actualiza solo con resultados observados. Estado inicial:
**Fase 0 en curso**.

## Matriz

| Requisito | Estado | Evidencia | Prueba ejecutada | Resultado | Limitación | Manual pendiente |
|---|---|---|---|---|---|---|
| Carpeta/app base | Verificado automáticamente | workspace, icono y archivos Tauri/React | TypeScript, Vite, Cargo y Tauri build | ejecutable generado | ventana bloqueada por sandbox Codex | abrir UI con el `.bat` |
| Migración inicial | Verificado automáticamente | `0001_initial.sql` | unittest, aplicación doble y Cargo | correcto | apertura manual pendiente | no |
| Integridad SQLite | Verificado automáticamente | `test_foundations.py` | `integrity_check` + `foreign_key_check` | `ok`, sin violaciones | usa sqlite3 del sistema | no |
| Adaptador Broker aislado | Verificado por compilación | `src/broker` | Cargo check/test/clippy | correcto | conexión UI pendiente | no |
| Conexión Broker | Verificado manualmente | Broker activo en `192.168.1.52:8765` | probe ejecutado en A9 | `live`, readiness `healthy`, contrato 2.5 | ejecución delegada por restricción LAN de Codex | integrar desde Tauri |
| Creación asíncrona | Verificado manualmente | tarea `task_612f1f5e873d43e3a99454b5685ace62` | `POST /api/v1/tasks` | HTTP 202 | no ejecutado desde binario Tauri | repetir desde app |
| Idempotencia remota | Verificado manualmente | mismo payload y clave | segundo `POST /api/v1/tasks` | HTTP 200 y mismo `task_id` | una ejecución real | automatizar en integración |
| Polling real | Verificado manualmente | timeline del probe | `GET /api/v1/tasks/{id}` | `queued → generating → completed` | no se observó reinicio | probar recuperación |
| Resultado remoto | Verificado manualmente | respuesta terminal | snapshot final | `HasResult=True`, sin error | contenido no archivado | inspección desde app |
| Polling | Compilado + Broker manual | política con backoff/jitter y estados reales | Cargo + probe remoto + tests contractuales | Broker completó `queued → generating → completed` | falta recorrerlo desde la ventana | ejecutar desde Tauri |
| Recuperación | Verificado automáticamente (persistencia) | consulta compartida Rust/test | unittest con identidad remota, request e idempotency key | sobreviven y solo activos pasan a recuperación | no se cerró Tauri real | cierre/reapertura |
| Turno de chat | Verificado automáticamente (persistencia) | conversación, mensajes, tarea y snapshot | unittest transaccional | commit completo y rollback sin huérfanos | Rust/React sin compilar | enviar desde la app |
| Contexto trazable | Verificado automáticamente | `context_snapshots` y `context_sources` | unittest de ventana | estrategia, fuente, razón y extracto persistidos | sin medición real de tokens | inspección desde app |
| Respuesta durable | Verificado automáticamente (persistencia) | mensaje asistente y resultado de tarea | doble materialización + reapertura SQLite | una sola parte y contenido preservado | polling Rust sin ejecutar | completar turno y reiniciar |
| Cancelación | Revisado estáticamente | comando Tauri y `DELETE` tipado | no ejecutada contra Broker | pendiente | prueba remota completó antes de cancelar | cancelar tarea lenta |
| Secretos | Verificado automáticamente (valores) | solo env; settings públicos | unittest + escaneo de valores | sin coincidencias | almacén OS pendiente | revisar logs |
| Empaquetado | No verificado | configuración MSI/NSIS | no ejecutada | — | Rust/deps ausentes | instalar paquete |

## Comandos ejecutados

```powershell
python -m unittest discover -s tests -v
```

Resultado final tras añadir regresiones de configuración de build: 13 pruebas
Python y 2 pruebas Rust, todas correctas.

También se ejecutaron correctamente:

- `tsc -b --pretty false`;
- `vite build`;
- `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`;
- `tauri build --no-bundle`, que generó `target/release/chatygpt.exe`.
La primera ejecución detectó un
archivo SQLite no cerrado por la propia prueba en Windows; se corrigió el cierre
explícito y se repitió la suite completa.

```powershell
python -  # parseo de todos los JSON del repositorio
python -  # parseo de todos los TOML con tomllib
```

Resultados: 7 JSON y 1 TOML válidos.

```powershell
python -  # TaskCreateRequest.model_validate_json contra app.schemas de AI Broker
```

Resultado: fixture válido; estrategia `single`, clasificación `local_only`,
`cloud_allowed=False`, `max_cost_usd=0.0`.

```powershell
python scripts\verify_broker.py --base-url http://127.0.0.1:8765 --smoke-task
```

Ejecutado en `A9_Mega`. Resultado:

- salud `live` y readiness `healthy`;
- contrato 2.5;
- 73 modelos despachables;
- creación HTTP 202;
- repetición idempotente HTTP 200 con el mismo `task_id`;
- estados observados `queued → generating → completed`;
- resultado presente y error ausente.

El probe tiene además pruebas con servidor contractual local que cubren salud,
capacidades, modelos, OpenAPI, creación 202, repetición 200, mismo `task_id` y
polling hasta `completed`.

```powershell
Invoke-WebRequest http://127.0.0.1:8765/openapi.json -TimeoutSec 3
```

Primer resultado: sin conexión porque AI Broker no estaba ejecutándose. Tras
arrancarlo en `A9_Mega`, la sesión aislada de Codex siguió sin permitir TCP a
la LAN y el navegador anfitrión rechazó direcciones privadas. El probe se
ejecutó por ello directamente en A9 y completó el recorrido remoto.

Rust estable, Cargo y las dependencias TypeScript quedaron instalados durante
la preparación del entorno. La compilación se verificó posteriormente con los
comandos y resultados indicados arriba.

La construcción aislada de `create_app().openapi()` se intentó con base, logs e
ingesta efímeros bajo `C:\tmp`; agotó el timeout de 30 segundos sin producir
contrato. Queda pendiente obtener el OpenAPI desde una instancia real.
