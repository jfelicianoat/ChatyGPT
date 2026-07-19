# ChatyGPT

Aplicación de escritorio Windows, local-first, para conversar mediante AI Broker
sin acoplar la interfaz a su API HTTP.

## Estado

Fase 0 en curso. El primer slice incluye:

- shell Tauri 2 + React + TypeScript;
- SQLite local con migración inicial y recuperación de tareas activas;
- adaptador tipado de AI Broker 2.5;
- diagnóstico de salud y capacidades sin crear inferencias;
- fixture contractual local-only y sin coste cloud;
- pruebas ejecutables con la biblioteca estándar de Python.

La conversación todavía está deshabilitada deliberadamente: antes se completará
el recorrido durable `persistir → crear → sondear → recuperar`.

## Desarrollo

Requisitos pendientes de instalar en la máquina auditada:

- Rust compatible con Tauri 2 (la documentación oficial exige al menos 1.77.2
  para los plugins SQL/Stronghold);
- dependencias JavaScript del workspace.

Cuando estén disponibles:

```powershell
pnpm.cmd install
pnpm.cmd typecheck
pnpm.cmd test
pnpm.cmd tauri dev
```

Las pruebas de fundamentos, que no requieren dependencias externas:

```powershell
python -m unittest discover -s tests -v
```

Configuración no secreta:

- `CHATYGPT_BROKER_BASE_URL`, por defecto `http://127.0.0.1:8765`.

Secreto de transición:

- `AI_BROKER_ADMIN_TOKEN`, leído solo del entorno y enviado como
  `x-admin-token`. No se persiste ni se registra.

Antes de distribuir la aplicación se sustituirá el entorno por Windows
Credential Manager o Stronghold, tras decidir el modelo de desbloqueo.

## Documentación

- [Arquitectura y plan](docs/ARCHITECTURE.md)
- [Evidencias de Fase 0](docs/PHASE_0_VERIFICATION.md)
- [Contrato local AI Broker 2.5](contracts/broker/2.5/single-task.request.json)

