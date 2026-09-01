# Resumen ejecutivo
- Proyecto de escritorio Windows local-first con Tauri 2, Rust, React 19, TypeScript y SQLite.
- La frontera de confianza está bien planteada: React no posee secretos ni acceso directo a SQLite/Broker; Rust concentra persistencia, red, filesystem y secretos.
- No se observan secretos API hardcodeados en los ficheros revisados; los tokens se gestionan en Rust y existe soporte DPAPI.
- La principal deuda es de mantenibilidad: `apps/desktop/src/App.tsx` ronda 8.6k líneas, `apps/desktop/src-tauri/src/db/mod.rs` ronda 18k y `lib.rs` supera 3.3k.
- `App.tsx` está excluido explícitamente del umbral de cobertura de Vitest; hay tests de componente, pero el archivo de mayor superficie UI no participa en el quality gate porcentual.
- El proyecto tiene una CI razonable (fmt, clippy, cobertura Rust, typecheck, Vitest, build y unittest Python), pero carece de auditoría automática de dependencias/secretos y de un gate de empaquetado MSI/NSIS.
- La configuración usa `sourcemap: true` en producción. En una app desktop esto no es una vulnerabilidad por sí sola, pero aumenta la exposición del código distribuido y debe ser una decisión explícita de release.
- Hay valores de infraestructura local hardcodeados en scripts (`192.168.1.52:8765`), lo que mezcla entorno con código y complica portabilidad.
- El acceso nativo mediante PowerShell/`reg.exe`/`explorer.exe` está encapsulado y existen pruebas de escape en algunas rutas; aun así, por su impacto debe permanecer como zona de revisión prioritaria.
- El ZIP contiene `graphify-out/` con numerosos artefactos generados y cachés. Deben mantenerse fuera del dominio de código fuente y preferiblemente fuera del repositorio si no son artefactos deliberadamente versionados.

# Mapa del sistema
```text
React 19 + TypeScript (apps/desktop/src)
  -> invoke/eventos Tauri
Rust core (apps/desktop/src-tauri/src)
  -> SQLite + 23 migraciones
  -> AI Broker HTTP
  -> Athena HTTP/SSE
  -> filesystem / exportación / adjuntos
  -> secretos DPAPI / operaciones Windows
  -> scheduler / workflows / research
Contratos JSON (contracts/broker)
Tests TS/Rust/Python
CI GitHub Actions
```

Clasificación:
- Código: `apps/desktop/src`, `apps/desktop/src-tauri/src`, `scripts/*.py`, `scripts/*.ps1`.
- Config/build: `package.json`, `pnpm-lock.yaml`, `Cargo.toml`, `Cargo.lock`, `vite.config.ts`, `tsconfig*.json`, `tauri.conf.json`.
- Infra/CI: `.github/workflows/ci.yml`, scripts de arranque/parada.
- Persistencia: `apps/desktop/src-tauri/migrations/*.sql`.
- Contratos: `contracts/broker/*`.
- Tests: `apps/desktop/src/**/*.test.*`, tests Rust embebidos, `tests/*.py`.
- Docs: `README.md`, `docs/*`.
- Generados: `graphify-out/*`.

Entrypoints:
- Frontend: `apps/desktop/src/main.tsx` -> `App.tsx`.
- Tauri/Rust: `apps/desktop/src-tauri/src/main.rs` / `lib.rs`.
- CI: `.github/workflows/ci.yml`.
- Operación Windows: `Arrancar ChatyGPT.bat`, `scripts/Start-ChatyGPT.ps1`, scripts Athena.

# Cómo funciona
El usuario interactúa con React. La UI llama comandos tipados de Tauri; el núcleo Rust actúa como composition root y frontera privilegiada. Rust persiste el estado en SQLite, crea/sondea tareas del AI Broker, gestiona Athena, secretos, adjuntos, exportaciones, automatizaciones y workflows. La documentación declara SQLite como fuente de verdad y el vault como proyección. El diseño general es coherente con una aplicación local-first y con una frontera de seguridad fuera del WebView.

# Hallazgos por fichero
## apps/desktop/src/App.tsx
### Rol del fichero
Componente raíz y coordinador de gran parte del estado/flujo de UI.

### Hallazgos
| Severidad | Tipo | Impacto | Prob. | Riesgo | Evidencia | Recomendación | Cambio sugerido |
|---|---|---:|---:|---:|---|---|---|
| High | Maintainability/Architecture | 5 | 4 | 20 | ~8.6k líneas, ~118 funciones detectadas | Descomponer por feature y caso de uso | Extraer hooks/controladores y vistas por dominio: chat, GPT, memoria, scheduler, Athena, research, attachments |
| High | Testing | 4 | 4 | 16 | `vite.config.ts` excluye `src/App.tsx` del umbral de coverage | Reducir primero el tamaño y luego incorporarlo a gates gradualmente | Cobertura por feature; evitar perseguir % sobre un monolito |

## apps/desktop/src-tauri/src/db/mod.rs
### Rol del fichero
Persistencia SQLite, modelos/validaciones y operaciones de numerosos dominios.

### Hallazgos
| Severidad | Tipo | Impacto | Prob. | Riesgo | Evidencia | Recomendación | Cambio sugerido |
|---|---|---:|---:|---:|---|---|---|
| High | Architecture/Maintainability | 5 | 5 | 25 | ~18k líneas y >300 funciones detectadas | Separar repositorios por bounded context | `db/conversations.rs`, `db/attachments.rs`, `db/memory.rs`, `db/gpts.rs`, `db/scheduler.rs`, `db/workflows.rs`, manteniendo una fachada `Database` |
| Medium | Testing | 3 | 4 | 12 | Gran cantidad de tests embebidos en el mismo módulo | Mover tests de integración a módulos dedicados | Mantener unitarios locales solo para invariantes pequeñas |

## apps/desktop/src-tauri/src/lib.rs
### Rol del fichero
Composition root y amplia superficie de comandos Tauri.

### Hallazgos
| Severidad | Tipo | Impacto | Prob. | Riesgo | Evidencia | Recomendación | Cambio sugerido |
|---|---|---:|---:|---:|---|---|---|
| High | Architecture | 4 | 4 | 16 | ~3.3k líneas y ~157 funciones; mezcla comandos y utilidades nativas | Separar comandos por dominio | `commands/athena.rs`, `commands/gpt.rs`, `commands/files.rs`, etc.; `lib.rs` solo registra estado/comandos |
| Medium | Security/Maintainability | 4 | 3 | 12 | Varias invocaciones a PowerShell y `explorer.exe` | Centralizar ejecución nativa y validación | Un adaptador `windows_native` con argumentos tipados, sin construir scripts salvo necesidad |

## apps/desktop/src-tauri/src/secrets.rs
### Rol del fichero
Custodia y resolución de credenciales.

### Hallazgos
| Severidad | Tipo | Impacto | Prob. | Riesgo | Evidencia | Recomendación | Cambio sugerido |
|---|---|---:|---:|---:|---|---|---|
| Medium | Security | 5 | 2 | 10 | Uso de PowerShell para DPAPI y operaciones sensibles | Mantener pruebas de escape y minimizar tránsito por shell | Migrar a API Windows nativa/ crate mantenido si reduce superficie y dependencia de PowerShell |

## apps/desktop/src-tauri/src/research_tools.rs
### Rol del fichero
Validación y acceso HTTP para herramientas de investigación/API externa.

### Hallazgos
| Severidad | Tipo | Impacto | Prob. | Riesgo | Evidencia | Recomendación | Cambio sugerido |
|---|---|---:|---:|---:|---|---|---|
| Medium | Security | 5 | 2 | 10 | Existe validación explícita de URL y rechazo de redes privadas; zona SSRF sensible | Mantener política deny-by-default y tests exhaustivos IPv4/IPv6/DNS rebinding | Resolver destino y volver a validar IP efectiva antes de conectar si no se hace ya en la capa HTTP |

## vite.config.ts
### Rol del fichero
Build Vite y calidad Vitest.

### Hallazgos
| Severidad | Tipo | Impacto | Prob. | Riesgo | Evidencia | Recomendación | Cambio sugerido |
|---|---|---:|---:|---:|---|---|---|
| Medium | Testing | 4 | 4 | 16 | `App.tsx` y `platform.ts` excluidos del threshold | Convertir exclusiones permanentes en deuda explícita con objetivo | Gate por módulos extraídos |
| Low | Security/Release | 2 | 3 | 6 | `build.sourcemap: true` | Decidir por perfil de release | Deshabilitar en distribución final o publicar sourcemaps solo en canal controlado |

## .github/workflows/ci.yml
### Rol del fichero
Quality gates Windows y checks portables Linux.

### Hallazgos
| Severidad | Tipo | Impacto | Prob. | Riesgo | Evidencia | Recomendación | Cambio sugerido |
|---|---|---:|---:|---:|---|---|---|
| Medium | Security/Dependency | 4 | 3 | 12 | No hay `cargo audit`, auditoría npm/pnpm, secret scan ni SBOM | Añadir gates de supply chain | `cargo audit`, `pnpm audit` o equivalente controlado, secret scanning y SBOM |
| Medium | Reliability/Release | 3 | 3 | 9 | CI construye frontend pero no valida `tauri build` MSI/NSIS | Añadir job de packaging | Smoke de instalación/arranque en release candidate |
| Low | Reproducibility | 2 | 3 | 6 | `dtolnay/rust-toolchain@stable` es flotante | Fijar versión Rust/MSRV de release | `rust-toolchain.toml` + CI consistente |

## Arrancar ChatyGPT.bat / scripts/Start-ChatyGPT.ps1
### Rol del fichero
Arranque local y configuración de servicios.

### Hallazgos
| Severidad | Tipo | Impacto | Prob. | Riesgo | Evidencia | Recomendación | Cambio sugerido |
|---|---|---:|---:|---:|---|---|---|
| Medium | Config | 3 | 4 | 12 | Broker por defecto `http://192.168.1.52:8765` | Sacar entorno del repositorio | `.env.example`, parámetro obligatorio o config local no versionada |

## graphify-out/
### Rol del fichero
Artefactos de análisis/generación.

### Hallazgos
| Severidad | Tipo | Impacto | Prob. | Riesgo | Evidencia | Recomendación | Cambio sugerido |
|---|---|---:|---:|---:|---|---|---|
| Low | Maintainability/Repository hygiene | 2 | 4 | 8 | Gran número de JSON/MD/cache generados | No tratar como fuente y revisar necesidad de versionado | Ignorar cachés; conservar solo informes deliberados |

# Hallazgos transversales
1. **Arquitectura:** la separación WebView/Rust/SQLite es buena, pero dentro de cada lado hay “god modules” (`App.tsx`, `db/mod.rs`, `lib.rs`).
2. **Fiabilidad:** existen patrones durables, recuperación e idempotencia en documentación y código; no se ejecutaron pruebas en esta auditoría.
3. **Seguridad:** CSP restrictiva salvo estilos inline, ACL Tauri mínima (`core:default`), secretos en Rust y validación de URLs. La superficie de PowerShell y herramientas web requiere controles continuos.
4. **Rendimiento:** no se hizo profiling. El principal riesgo observable por lectura es complejidad de UI/DB, no un hotspot demostrado.
5. **Observabilidad:** hay módulo `logging.rs` y `metrics.rs`; buena base. Falta convertirlo en gate operacional/release.
6. **Tests:** cobertura amplia en Rust/TS/Python, pero el componente UI más grande está fuera del threshold.
7. **Configuración:** parte de entorno está en scripts con IP fija.
8. **Dependencias:** lockfiles presentes y CI usa `--frozen-lockfile`; falta auditoría automática de vulnerabilidades/licencias/SBOM.

# Estándares recomendados
- Máximo orientativo de 500-800 líneas por módulo de aplicación; excepciones justificadas.
- Commands Tauri organizados por dominio.
- Repositorios SQLite por dominio, con transacciones coordinadas desde servicios.
- React: feature folders + hooks de caso de uso + componentes de presentación.
- Rust: `rust-toolchain.toml`, fmt/clippy/audit, deny warnings.
- JS: mantener lockfile, Renovate/Dependabot, audit controlado, Vitest 4+.
- ADRs para decisiones de seguridad (DPAPI/PowerShell, SSRF, sourcemaps).
- Logs estructurados sin contenido sensible, con correlation IDs.
- CI de release con empaquetado Tauri y smoke de artefacto.

# Roadmap
## Quick wins
- Eliminar IP de Broker hardcodeada.
- Añadir auditoría de dependencias y secret scanning a CI.
- Fijar toolchain Rust.
- Decidir política de sourcemaps.
- Marcar `graphify-out/cache` como generado/no versionable si aplica.

## Medio plazo
- Extraer 3-5 dominios de `App.tsx`.
- Dividir `db/mod.rs` por repositorios.
- Dividir `lib.rs` en módulos de comandos.
- Añadir packaging MSI/NSIS a CI.

## Largo plazo
- Introducir servicios de aplicación explícitos entre commands y repositorios.
- Contract/integration tests por frontera Broker/Athena.
- Observabilidad de release y runbooks.

# Tareas para ejecución
- AUD-001 (High, riesgo 20, L): descomponer `App.tsx`.
- AUD-002 (High, riesgo 25, L): modularizar `db/mod.rs`.
- AUD-003 (High, riesgo 16, M): separar comandos de `lib.rs`.
- AUD-004 (Medium, riesgo 12, S): supply-chain gates en CI.
- AUD-005 (Medium, riesgo 12, S): externalizar Broker URL.
- AUD-006 (Medium, riesgo 9, M): packaging Tauri en CI.
- AUD-007 (Medium, riesgo 10, M): revisar/encapsular ejecución PowerShell.
- AUD-008 (Low, riesgo 6, S): política de sourcemaps.

# Supuestos y límites del análisis
- Análisis por lectura estática del ZIP suministrado; no se ejecutó la aplicación, build, tests, linters ni scanners.
- El repositorio GitHub fue consultado únicamente para confirmar la estructura/estado público general; el ZIP adjunto se tomó como fuente primaria del código.
- No se afirma presencia de CVEs sin referencia externa.
- Por tamaño del proyecto no se enumeran los ~1.5k símbolos detectados uno a uno. Los planes de upgrade incluyen PHASE_0 y un plan por módulos para completar touchpoints.
