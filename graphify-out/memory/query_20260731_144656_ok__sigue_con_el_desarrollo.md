---
type: "query"
date: "2026-07-31T14:46:56.277106+00:00"
question: "Ok, sigue con el desarrollo"
contributor: "graphify"
outcome: "useful"
source_nodes: ["App()", "startup.rs", "protect_token()", "refresh_protected_token_if_enabled()", "BrokerClient", "Database"]
---

# Q: Ok, sigue con el desarrollo

## Answer

Se añadió un inicio con Windows reversible desde Tareas programadas. Rust registra HKCU Run tras confirmación, protege el token del Broker con DPAPI CurrentUser, genera un script sin secretos que espera una respuesta autenticada de capabilities y evita instancias duplicadas. La interfaz muestra estado y permite activar o desactivar; el cambio se audita sin credenciales. Se verificó con 87 pruebas Rust, 51 frontend, 15 Python, clippy, TypeScript, Vite y una compilación Tauri release.

## Outcome

- Signal: useful

## Source Nodes

- App()
- startup.rs
- protect_token()
- refresh_protected_token_if_enabled()
- BrokerClient
- Database