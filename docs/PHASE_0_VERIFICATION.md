# Evidencias de Fase 0

Este documento se actualiza solo con resultados observados. Estado inicial:
**Fase 0 en curso**.

## Matriz

| Requisito | Estado | Evidencia | Prueba ejecutada | Resultado | Limitación | Manual pendiente |
|---|---|---|---|---|---|---|
| Carpeta/app base | Revisado estáticamente | workspace y archivos Tauri/React | listado de archivos | creada | dependencias no instaladas | abrir UI |
| Migración inicial | Verificado automáticamente | `0001_initial.sql` | unittest, aplicación doble | correcto | compilación Rust pendiente | no |
| Integridad SQLite | Verificado automáticamente | `test_foundations.py` | `integrity_check` + `foreign_key_check` | `ok`, sin violaciones | usa sqlite3 del sistema | no |
| Adaptador Broker aislado | Revisado estáticamente | `src/broker` | revisión | presente | Rust sin compilar | no |
| Conexión Broker | No verificado | servicio apagado | GET OpenAPI | sin conexión | Broker no iniciado | iniciar servicio |
| Polling | Revisado estáticamente | política con backoff | Rust test pendiente | no ejecutada | cargo ausente | no |
| Recuperación | Verificado automáticamente (local) | consulta compartida Rust/test | unittest con estados activos y terminales | correcto | reconciliación remota incompleta | cierre/reapertura |
| Secretos | Verificado automáticamente (valores) | solo env; settings públicos | unittest + escaneo de valores | sin coincidencias | almacén OS pendiente | revisar logs |
| Empaquetado | No verificado | configuración MSI/NSIS | no ejecutada | — | Rust/deps ausentes | instalar paquete |

## Comandos ejecutados

```powershell
python -m unittest discover -s tests -v
```

Resultado final: 5 pruebas, todas correctas. La primera ejecución detectó un
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
Invoke-WebRequest http://127.0.0.1:8765/openapi.json -TimeoutSec 3
```

Resultado: sin conexión; AI Broker no estaba ejecutándose.

```powershell
rustc --version
cargo --version
tsc --version
```

Resultado: no disponibles. Por ello no se afirma que Tauri o TypeScript
compilen.

La construcción aislada de `create_app().openapi()` se intentó con base, logs e
ingesta efímeros bajo `C:\tmp`; agotó el timeout de 30 segundos sin producir
contrato. Queda pendiente obtener el OpenAPI desde una instancia real.
