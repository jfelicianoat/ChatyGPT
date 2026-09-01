# Resumen ejecutivo de actualización (conservador)
Objetivo: recuperar alineación de soporte y reducir riesgo con cambios mínimos, sin re-arquitectura. El stack principal no está globalmente EOL: Tauri 2 y React 19 están actuales; Node 22 sigue LTS. El desfase claro está en tooling frontend: Vite 6 está dos majors detrás de Vite 8 y Vitest 3 detrás de Vitest 4 estable.

# Fuentes de versiones (FUENTES_DE_VERSION)
| Componente | Versión detectada | Fuente | Confianza | Comentario |
|---|---|---|---|---|
| Node CI | 22 | `.github/workflows/ci.yml` | Alta | Línea LTS vigente |
| pnpm | 11.9.0 | `package.json`, CI | Alta | Fijado por packageManager/CI |
| React | 19.2.7 | `pnpm-lock.yaml` | Alta | Manifest permite ^19 |
| React DOM | 19.2.7 | `pnpm-lock.yaml` | Alta | |
| TypeScript | 5.9.3 | `pnpm-lock.yaml` | Alta | |
| Vite | 6.4.3 | `pnpm-lock.yaml` | Alta | 2 majors detrás de Vite 8 |
| Vitest | 3.2.7 | `pnpm-lock.yaml` | Alta | 1 major detrás de estable 4 |
| Tauri Rust | 2.11.5 | `Cargo.lock` | Alta | Coincide con release estable 2.11.5 consultada |
| @tauri-apps/api | 2.11.1 | `pnpm-lock.yaml` | Alta | Actual en la rama 2.11 consultada |
| @tauri-apps/cli | 2.11.4 | `pnpm-lock.yaml` | Alta | Actual en la rama 2.11 consultada |
| reqwest | 0.12.28 | `Cargo.lock` | Alta | |
| rusqlite | 0.37.0 | `Cargo.lock` | Alta | |
| tokio | 1.53.0 | `Cargo.lock` | Alta | |
| Python CI | 3.13 | CI | Alta | Solo tests/scripts |
| jsonschema | 4.26.0 | `tests/requirements.txt` | Alta | Única dependencia Python declarada de tests |

# Referencias (EOL/soporte/CVEs/breaking changes) con fecha consultada
Fecha consultada: 2026-09-01.

- Node.js: la tabla oficial marca Node 24 y Node 22 como LTS; Node 20 aparece EOL. Fuente: https://nodejs.org/en/about/previous-releases
- Node publicó releases de seguridad en junio de 2026 para 22/24/26, con severidad máxima HIGH. Fuente: https://nodejs.org/en/blog/vulnerability/june-2026-security-releases
- Vite 7 elevó el requisito a Node 20.19+ / 22.12+ y retiró Node 18. Fuente: https://vite.dev/blog/announcing-vite7 y guía de migración v6->v7.
- Vite 8 estable fue anunciado el 2026-03-12 y sustituye internamente esbuild/Rollup por Rolldown; mantiene Node 20.19+ / 22.12+. Fuente: https://vite.dev/blog/announcing-vite8
- Vitest 4 estable fue anunciado el 2025-10-22. Su guía de migración indica que requiere Vite >=6 y Node >=20. Fuente: https://vitest.dev/blog/vitest-4 y https://vitest.dev/guide/migration
- Tauri core 2.11.5 fue publicado el 2026-07-01. La rama 2.11.1 incluyó fixes de seguridad de ACL/orígenes remotos; el lock actual ya está por encima de esa versión. Fuente: https://tauri.app/release/tauri/all-versions/ y release 2.11.1.
- No se declara ningún CVE específico para las versiones locked de crates/npm porque esta auditoría no ejecutó scanners y no se obtuvo evidencia oficial específica suficiente.

# Matriz de obsolescencia (MATRIZ_DE_OBSOLESCENCIA)
| Área | Estado | Evidencia | Consecuencia | Acción |
|---|---|---|---|---|
| Node | OK | CI Node 22; LTS oficial | Baja | Mantener 22 con patch actualizado |
| React | OK | 19.2.7 | Baja | Mantener |
| Tauri | OK | 2.11.5 lock | Baja | Mantener 2.11.x |
| Vite | Obsoleto según criterio C | 6.4.3; Vite 8 estable, 2 majors detrás | Soporte/ecosistema y migración futura más costosa | Subir al menos a 7.x |
| Vitest | Riesgo | 3.2.7; 4 estable | Divergencia tooling | Subir a 4.x |
| TypeScript | OK | 5.9.3 | Baja | Mantener |
| Rust toolchain | Riesgo | CI usa `stable` flotante | Reproducibilidad | Fijar toolchain |
| Supply chain | Riesgo | Sin audit automático | Vulnerabilidades pueden entrar sin gate | Añadir scans |
| Packaging | Riesgo | No hay `tauri build` en CI | Release no verificado end-to-end | Añadir gate |

# Targets recomendados (mínimo viable) y justificación
- Node: mantener 22 LTS, pero fijar mínimo >=22.12 para compatibilidad con Vite 7/8.
- Vite: 7.x como target conservador. Es un salto único desde 6 y evita cambiar de bundler internamente a Rolldown en el mismo paso.
- Vitest: 4.x estable.
- React/React DOM: mantener 19.2.x.
- Tauri: mantener 2.11.x actualizando patch si aparece.
- Rust crates: mantener majors; actualizar patches/minors compatibles mediante lockfile.
- Python: mantener 3.13 en CI salvo necesidad real.
- pnpm: mantener 11.9.0 durante este upgrade para limitar variables.

# Plan de cambios por área (runtime/deps/toolchain/config/CI/infra)
## Runtime/lenguaje
1. Añadir `.nvmrc`/`.node-version` o `engines.node >=22.12 <23` si el proyecto quiere estandarizar Node 22.
2. Añadir `rust-toolchain.toml` con una versión estable concreta comprobada.
3. Mantener Python 3.13 en CI.

## Dependencias
1. `vite` 6 -> 7.
2. `vitest` y `@vitest/coverage-v8` 3 -> 4.
3. Alinear `@vitejs/plugin-react` con Vite 7.
4. Mantener React/Tauri majors.
5. Regenerar lockfile y revisar diff.

## Toolchain/build
- Validar cambios de browser target de Vite 7.
- Confirmar que `vitest/config` y configuración de coverage siguen válidos.
- No cambiar Rolldown/Vite 8 en el plan conservador.

## Config
- Sacar `192.168.1.52:8765` a configuración local.
- Mantener CSP actual; documentar `unsafe-inline` en `style-src`.

## CI/CD
- Añadir `cargo audit`.
- Añadir auditoría pnpm controlada.
- Secret scan.
- Fijar toolchains.
- Añadir `tauri build` opcional en job release.

## Infra
No hay Docker/Kubernetes/Terraform detectados en el ZIP.

# Touchpoints de cambio (TOUCHPOINTS_DE_CAMBIO)
## PHASE_0
| Ruta/módulo | Símbolo/área | Cambio esperado | Riesgo | Test |
|---|---|---|---|---|
| `package.json` | devDependencies | Vite/Vitest/plugin-react | Medium | `pnpm typecheck`, `pnpm test`, `pnpm build` |
| `pnpm-lock.yaml` | lock | Regeneración | Medium | install frozen en CI |
| `vite.config.ts` | `defineConfig` | Compatibilidad Vite 7/Vitest 4 | Medium | coverage + build |
| `.github/workflows/ci.yml` | Node/tooling | fijar mínimos y audit | Medium | workflow |
| `apps/desktop/index.html` | frontend entry | verificar output Vite | Low | smoke |
| `apps/desktop/src/main.tsx` | entry | verificar bootstrap | Low | component smoke |
| `apps/desktop/src/App.tsx` | root UI | no refactor; solo compatibilidad | High por tamaño | tests existentes + smoke |
| `apps/desktop/src/**/*.test.*` | tests | ajustes por Vitest 4 si aparecen | Medium | suite Vitest |
| `apps/desktop/src-tauri/Cargo.toml` | deps | solo patches/minors compatibles | Medium | cargo test/clippy |
| `apps/desktop/src-tauri/Cargo.lock` | lock | actualización controlada | Medium | cargo test |
| `.github/workflows/ci.yml` | release | añadir audit/packaging | Medium | CI |

## PLAN_POR_FASES
- Fase C1: manifests, lockfiles, Vite/Vitest.
- Fase C2: CI/toolchains/audits.
- Fase C3: frontend por carpeta `apps/desktop/src`.
- Fase C4: Rust por paquete `src-tauri/src`.
- Fase C5: scripts/tests/docs y empaquetado.
- Para completar el 100% de ~1.5k símbolos detectados, revisar lotes por módulo y registrar únicamente los que fallen por breaking change.

## NEXT_PHASE_ASK
Siguiente lote recomendado si se continúa de forma exhaustiva: `apps/desktop/src` (frontend) o `apps/desktop/src-tauri/src` (Rust), elegido por carpeta.

# Roadmap (Quick wins / Medio / Largo)
## Quick wins
- Fijar Node mínimo y Rust toolchain.
- Externalizar Broker URL.
- Añadir audits.
- Upgrade Vitest 4.

## Medio
- Upgrade Vite 7 y resolver cambios.
- Validar todo el frontend y contratos.
- Packaging Tauri en CI.

## Largo
- Saltar a Vite 8 solo después de estabilizar el conservador.

# TAREAS_UPGRADE_CONSERVADOR
## UGC-001 — Fijar runtimes
- Archivos: `.github/workflows/ci.yml`, nuevo `rust-toolchain.toml`, `package.json`.
- Pasos: fijar Node >=22.12; fijar Rust estable concreta.
- Aceptación: CI usa exactamente los mismos runtimes local/release.
- Prioridad: High. Severidad: Medium. Riesgo: 12. Esfuerzo: S.

## UGC-002 — Vitest 4
- Archivos: `package.json`, `pnpm-lock.yaml`, `vite.config.ts`, tests afectados.
- Pasos: bump; revisar migration guide; ajustar coverage.
- Aceptación: tests y threshold pasan sin excluir más código.
- Dependencia: UGC-001.
- Prioridad: High. Riesgo: 12. Esfuerzo: M.

## UGC-003 — Vite 7
- Archivos: `package.json`, lock, `vite.config.ts`, `index.html`.
- Pasos: bump; revisar browser target y deprecated APIs.
- Aceptación: build de producción y Tauri dev/build correctos.
- Dependencia: UGC-001.
- Prioridad: High. Riesgo: 12. Esfuerzo: M.

## UGC-004 — Supply chain gates
- Archivos: `.github/workflows/ci.yml`.
- Pasos: cargo audit; audit npm/pnpm; secret scan; opcional SBOM.
- Aceptación: PR falla ante vulnerabilidad por encima de política definida.
- Prioridad: High. Riesgo: 12. Esfuerzo: S.

## UGC-005 — Externalizar Broker URL
- Archivos: BAT/PS1/scripts de diagnóstico.
- Pasos: parámetro/config local; default loopback o error explícito.
- Aceptación: ninguna IP LAN personal queda como default.
- Prioridad: Medium. Riesgo: 8. Esfuerzo: S.

## UGC-006 — Gate de empaquetado
- Archivos: CI, `tauri.conf.json`.
- Pasos: job `tauri build`; conservar artefactos; smoke mínimo.
- Aceptación: MSI/NSIS se construyen reproduciblemente.
- Prioridad: Medium. Riesgo: 9. Esfuerzo: M.

# Verificación y checklist post-upgrade
- [ ] `pnpm install --frozen-lockfile`
- [ ] `pnpm typecheck`
- [ ] `pnpm test:coverage`
- [ ] `pnpm build`
- [ ] `cargo fmt --check`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test`
- [ ] `python -m unittest discover -s tests -v`
- [ ] dependency audit
- [ ] secret scan
- [ ] `tauri build` Windows
- [ ] smoke de arranque, chat, adjunto, Athena, scheduler y exportación

# Supuestos y límites
- No se ejecutaron comandos ni scanners.
- El target conservador prioriza mínimo cambio; por ello no propone Vite 8/Rolldown ni refactors estructurales.
- CVEs concretos deben confirmarse con scanner y advisory oficial en la fecha de ejecución.
