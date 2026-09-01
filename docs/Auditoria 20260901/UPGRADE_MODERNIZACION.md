# Resumen ejecutivo de modernización
La modernización debe atacar dos problemas distintos: tooling frontend atrasado (Vite 6/Vitest 3) y concentración arquitectónica extrema en `App.tsx`, `db/mod.rs` y `lib.rs`. El objetivo no es reescribir, sino introducir boundaries claros, calidad reproducible, observabilidad y release hardening.

# Diferencias clave vs plan conservador
- Vite 8 en lugar de Vite 7.
- Node 24 LTS como runtime recomendado de desarrollo/CI.
- Vitest 4 estable (Vitest 5 estaba en beta en la documentación consultada; no se recomienda como target estable).
- Refactor por features/repositorios/comandos.
- Quality gates de supply chain y packaging.
- Mejorar tests de integración y cobertura de UI por módulos extraídos.
- Encapsular operaciones Windows/PowerShell y reforzar SSRF/egress.
- Observabilidad y métricas orientadas a release.

# Targets recomendados (modernización) y justificación
- Node: 24 LTS. La tabla oficial de Node lo marca LTS a 2026-09-01.
- pnpm: mantener 11.x al principio; actualizar minor/patch tras estabilizar.
- React: 19.2.x.
- Vite: 8.x estable. Aporta Rolldown como bundler unificado; el cambio debe ir aislado.
- Vitest: 4.x estable. No usar Vitest 5 beta como baseline.
- TypeScript: 5.9.x o estable compatible más reciente verificado en el momento del cambio.
- Tauri: 2.11.x o última 2.x compatible; mantener lock coherente.
- Rust edition: no cambiar por moda. Evaluar Edition 2024 en una fase separada después del refactor si MSRV/toolchain lo permiten.
- SQLite/rusqlite/reqwest/tokio: actualizar dentro de majors compatibles primero; majors nuevos solo con evidencia de valor.

# Plan por áreas (arquitectura/observabilidad/tests/CI/deps/infra)
## Arquitectura
### Frontend
Transformar `App.tsx` en shell:
```text
src/
  app/
  features/chat/
  features/gpts/
  features/memory/
  features/attachments/
  features/scheduler/
  features/research/
  features/athena/
  features/workflows/
  shared/
```
Cada feature: tipos, hook/controlador, vista, tests.

### Rust
```text
src/
  commands/
  application/
  domain/
  infrastructure/
    db/
    broker/
    athena/
    windows/
```
No es necesario aplicar DDD ceremonial. El objetivo es separar comandos, casos de uso y adaptadores.

### Persistencia
Dividir `db/mod.rs` sin cambiar el esquema inicialmente. Introducir módulos de repositorio por dominio detrás de la misma conexión/transacción.

## Observabilidad
- Mantener logging estructurado existente.
- Definir eventos de negocio no sensibles y métricas: latencia Broker/Athena, recovery, ingestión, workflow, scheduler.
- Añadir correlation ID end-to-end.
- Política de redacción automatizada en tests.

## Tests
- Tests de integración por comando/caso de uso Rust.
- Tests de contrato Broker/Athena con fixtures versionados.
- Component tests por feature React.
- Smoke de paquete MSI/NSIS.
- Reducir exclusión de `App.tsx` conforme se vacíe.
- Mutation testing selectivo solo en lógica crítica, no como gate global.

## CI
- Fijar toolchains.
- Caches con claves robustas.
- cargo audit / dependency scan / secret scan.
- SBOM.
- build + package.
- quality gates por módulo.
- Dependabot/Renovate con PRs pequeñas.

## Dependencias
- Fase separada para Vite 8 por el cambio a Rolldown.
- Vitest 4 antes o junto al cambio de Vite, pero con commit independiente.
- Alinear Tauri JS/Rust 2.11.x.
- Revisar crates transitivos con advisory scanner.

## Infra
No se detectaron contenedores/orquestación. El “infra” relevante es Windows local, WebView2, Broker/Athena y GitHub Actions.

# Touchpoints
## PHASE_0
### Frontend crítico
- `apps/desktop/src/App.tsx`: shell y extracción progresiva.
- `apps/desktop/src/platform.ts`: separar puertos por feature.
- `apps/desktop/src/domain.ts`: repartir lógica por dominio.
- `AthenaArea.tsx`, `WorkflowStudio.tsx`: candidatos a feature modules.
- `vite.config.ts`: Vite 8/Vitest 4 y nueva política coverage.
- tests de cada feature.

### Rust crítico
- `src-tauri/src/lib.rs`: commands -> módulos.
- `src-tauri/src/db/mod.rs`: repositorios por dominio.
- `task_runtime.rs`: servicio de aplicación de chat/research.
- `workflow_runtime.rs`: boundary workflow.
- `attachment_runtime.rs`: boundary attachments.
- `broker/*`, `athena/*`: adapters externos.
- `secrets.rs`, `startup.rs`, operaciones PowerShell: adapter Windows.
- `research_tools.rs`: egress/SSRF.
- `logging.rs`, `metrics.rs`: observabilidad.

### Build/release
- `package.json`, lockfiles, Cargo manifests.
- `.github/workflows/ci.yml`.
- `tauri.conf.json`.
- BAT/PowerShell.
- migraciones y contratos como artefactos de compatibilidad.

## PLAN_POR_FASES
### Fase 0 — Fundación
1. Fijar Node 24/Rust.
2. Añadir audits, secret scan, SBOM y packaging.
3. Capturar baseline de tests/cobertura.
4. Definir ADRs de boundaries y política de release.
5. Externalizar configuración de Broker/Athena.

### Fase 1 — Upgrades mayores
1. Vitest 4.
2. Vite 8.
3. Alinear plugin React.
4. Actualizar deps Rust dentro de compatibilidad.
5. Validar Tauri package.

### Fase 2 — Refactors/deuda
1. Extraer frontend por features.
2. Extraer commands Rust.
3. Separar DB repositories.
4. Separar adapter Windows.
5. Reducir `App.tsx`, `lib.rs`, `db/mod.rs` a composition/fachadas.

### Fase 3 — Hardening
1. SSRF/egress con resolución DNS validada y bloqueo de rangos privados tras resolución.
2. Pruebas de redacción de logs.
3. Release smoke MSI/NSIS.
4. Backups/migration failure tests de SQLite.
5. Fault injection en Broker/Athena y recuperación.
6. Performance budgets.

## NEXT_PHASE_ASK
Para continuar el inventario símbolo-a-símbolo, seleccionar una carpeta:
- `apps/desktop/src` (frontend), o
- `apps/desktop/src-tauri/src` (Rust).
El análisis debe avanzar por carpeta/módulo, no por archivos sueltos arbitrarios.

# Roadmap (fases claras)
## Quick wins
- Supply-chain gates.
- Runtime/toolchain pinning.
- Config externa.
- Política sourcemaps.
- `graphify-out` hygiene.

## Medio
- Vite 8/Vitest 4.
- Packaging CI.
- Primeras extracciones de `App.tsx` y `db/mod.rs`.

## Largo
- Arquitectura modular completa.
- Hardening egress/native Windows.
- Contract/integration tests y observabilidad de release.

# TAREAS_UPGRADE_MODERNIZACION
## UGM-001 — Baseline reproducible
- Descripción: fijar Node 24 LTS, Rust toolchain y comandos de verificación.
- Archivos: CI, `package.json`, nuevo `rust-toolchain.toml`, docs.
- Aceptación: misma toolchain en local/CI/release.
- Prioridad: Critical path. Severidad: Medium. Riesgo: 12. Esfuerzo: S.

## UGM-002 — Supply-chain hardening
- Archivos: CI.
- Pasos: cargo audit, pnpm audit/política equivalente, secret scan, SBOM.
- Aceptación: gates documentados y reproducibles.
- Prioridad: High. Severidad: High. Riesgo: 16. Esfuerzo: M.

## UGM-003 — Migrar a Vitest 4
- Archivos: package/lock/vite config/tests.
- Aceptación: cobertura estable, sin nuevas exclusiones.
- Prioridad: High. Riesgo: 12. Esfuerzo: M.

## UGM-004 — Migrar a Vite 8
- Archivos: package/lock/config/index.
- Pasos: revisar migration guide, plugins y output Rolldown.
- Aceptación: dev/build/test/Tauri package sin regresiones.
- Dependencia: UGM-003 recomendado.
- Prioridad: High. Severidad: High por toolchain. Riesgo: 16. Esfuerzo: M/L.

## UGM-005 — Feature decomposition de App.tsx
- Archivos: `App.tsx`, nuevos módulos feature.
- Pasos: extraer una feature por PR; conservar interfaces.
- Aceptación: App actúa como shell y cada feature tiene tests.
- Prioridad: High. Severidad: High. Riesgo: 20. Esfuerzo: L.

## UGM-006 — Modularizar persistencia
- Archivos: `db/mod.rs`, nuevos `db/*.rs`.
- Pasos: extraer repositories sin alterar SQL/schema; después introducir servicios.
- Aceptación: `db/mod.rs` deja de concentrar cientos de operaciones.
- Prioridad: High. Severidad: High. Riesgo: 25. Esfuerzo: L.

## UGM-007 — Modularizar commands
- Archivos: `lib.rs`, nuevos `commands/*.rs`.
- Aceptación: `lib.rs` queda como composition root/registro.
- Prioridad: High. Riesgo: 16. Esfuerzo: M.

## UGM-008 — Adapter Windows nativo
- Archivos: `secrets.rs`, `startup.rs`, `lib.rs`.
- Pasos: centralizar PowerShell/reg/explorer; tipar entradas; revisar si DPAPI puede invocarse sin shell.
- Aceptación: una sola superficie de ejecución nativa, tests de escaping y errores.
- Prioridad: High. Severidad: High. Riesgo: 15. Esfuerzo: M/L.

## UGM-009 — Hardening SSRF
- Archivos: `research_tools.rs`.
- Pasos: validar esquema/host/puerto; resolver DNS; rechazar IP privada/link-local/loopback tras resolución; controlar redirects.
- Aceptación: suite con IPv4, IPv6, hostname, redirect y DNS rebinding simulado.
- Prioridad: High. Severidad: High. Riesgo: 15. Esfuerzo: M.

## UGM-010 — Release pipeline
- Archivos: CI, Tauri config.
- Pasos: `tauri build`, artefactos, smoke, firma cuando exista.
- Aceptación: cada release candidate produce MSI/NSIS verificables.
- Prioridad: Medium. Riesgo: 9. Esfuerzo: M.

## UGM-011 — Cobertura UI por features
- Archivos: vite config/tests/features.
- Pasos: reducir exclusión a medida que App se vacía.
- Aceptación: lógica extraída participa del gate.
- Prioridad: Medium. Riesgo: 12. Esfuerzo: M.

## UGM-012 — Observabilidad operacional
- Archivos: `logging.rs`, `metrics.rs`, runtimes.
- Pasos: eventos y métricas estándar, redacción, correlation IDs.
- Aceptación: incidentes de Broker/Athena/workflow se pueden reconstruir sin exponer contenido sensible.
- Prioridad: Medium. Riesgo: 10. Esfuerzo: M.

# Verificación y checklist post-modernización
- [ ] install frozen
- [ ] typecheck
- [ ] Vitest coverage
- [ ] Rust fmt/clippy/tests/coverage
- [ ] Python tests
- [ ] contract tests Broker/Athena
- [ ] dependency/security scans
- [ ] SBOM
- [ ] Tauri build MSI/NSIS
- [ ] smoke de instalación/arranque
- [ ] recuperación tras reinicio
- [ ] adjuntos y exportación
- [ ] scheduler/workflows
- [ ] SSRF regression suite
- [ ] log redaction suite
- [ ] rollback documentado

# Supuestos y límites
- No se ejecutó el proyecto.
- Vite 8 es un cambio mayor de build y debe aislarse de los refactors.
- No se recomienda Vitest 5 mientras la documentación oficial consultada lo marque beta.
- La modernización preserva SQLite y Tauri; no hay evidencia que justifique reemplazarlos.
