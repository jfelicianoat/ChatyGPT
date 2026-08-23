# Compatibilidad de ChatyGPT con AI Broker

Revisión: **23 de agosto de 2026**.

Este documento describe al cliente ChatyGPT. No es la especificación de AI Broker ni
autoriza cambios en ese proyecto.

## Regla de compatibilidad

ChatyGPT valida de forma estricta los campos que necesita y tolera campos adicionales. El
cuerpo de petición sigue el baseline 2.8; el lector admite las extensiones aditivas 2.9.
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

ChatyGPT no envía `exclude_from_model_learning`: las conversaciones reales sí forman parte
del tráfico normal del producto. Esa bandera pertenece a clientes de evaluación como
Model_Drift.

Los fixtures bajo `contracts/broker/2.5` a `2.8` son capturas históricas y pruebas de
compatibilidad del cliente. El soporte de campos 2.9 está tipado y probado en
`apps/desktop/src-tauri/src/broker/contracts.rs`; la ausencia de una carpeta de fixture 2.9
no significa que el lector rechace esa revisión.

