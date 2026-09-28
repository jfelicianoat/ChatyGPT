# Evidencias de la auditoría del 28 de septiembre de 2026

Estas comprobaciones describen fallos del código revisado, commit `d0473ddc69b59bb01d500aef0e510e944c14f92a`.
No son correcciones del producto. Sus aserciones afirman el comportamiento defectuoso actual; después de corregirlo hay que convertirlas en pruebas de regresión con la expectativa correcta.

`ui.test.tsx` usa los componentes React reales y un puente `platform` simulado basado en el fixture existente de la aplicación. No accede a servicios reales. Reproduce traslado de borrador, respuesta antigua de navegación, navegación causada por un envío pendiente, restauración de un texto aceptado tras fallo de refresco, Enter durante composición IME y pérdida de cambios del editor de flujos. Resultado: **6 casos reproducidos**.

`sql_checks.py` aplica todas las migraciones en SQLite en memoria y extrae del código vigente las consultas relevantes. Confirma selección de una programación con ejecución activa, recuperación de una claim sin vínculo pese a una tarea existente, reapertura de un estado terminal por UPDATE tardío, borrado del historial por cascada y búsqueda sin normalización española. Resultado: **5 casos confirmados**. No ejecuta el runtime Rust ni simula un apagado físico.

Ejecutar desde la raíz del proyecto:

```powershell
rtk proxy node node_modules/vitest/vitest.mjs run docs/AUDIT_2026-09-28/evidence/ui.test.tsx --root .
rtk proxy python -X utf8 docs/AUDIT_2026-09-28/evidence/sql_checks.py
```

La configuración normal de Vitest tiene raíz `apps/desktop`, por lo que no incorpora automáticamente estas pruebas de auditoría. El argumento `--root .` las ejecuta expresamente. No deben añadirse sin adaptar sus expectativas a la suite habitual.

`finalize_report.py` comprueba las referencias iniciales del informe y convierte su notación de trabajo a enlaces absolutos. No modifica código de aplicación.

## Estado tras la corrección (0.3.0)

Las correcciones están descritas en [../RESOLUCION.md](../RESOLUCION.md). Estas evidencias
afirmaban el defecto, así que ahora **deben fallar**: las 6 de interfaz fallan y, de las 5
de SQL, fallan 3. Las otras 2 (`test_due_query_selects_task_with_active_run` y
`test_recovery_returns_claim_even_when_unlinked_chat_task_exists`) siguen pasando porque
comprueban una consulta aislada cuya corrección vive en la lógica que la rodea; las
regresiones con la expectativa correcta están en la suite normal
(`programacion_auditoria.rs`, `tests_auditoria.rs`, `App.auditoria.test.tsx`,
`WorkflowStudio.auditoria.test.tsx`).
