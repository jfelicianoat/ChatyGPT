# Cierre de la auditoría técnica del 13 de septiembre de 2026

Base revisada: `b30e0fe31b0cabe6601797ec112a461366158fec`. Este documento describe
las correcciones aplicadas sobre esa base. `CURRENT_STATE.md` es la referencia normativa;
las auditorías anteriores quedan como evidencia histórica.

## Resultado por hallazgo

| ID | Corrección aplicada | Evidencia automatizada |
| --- | --- | --- |
| H01 | Se reparó el tipo de artefactos en `App.tsx` y el estado vacío de su carga. | TypeScript, formato y build de Vite. |
| H02 | La investigación bloquea destinos privados tras resolver DNS y vuelve a comprobar cada redirección, incluidas IPv4 mapeadas en IPv6. | Pruebas de destinos privados, DNS y redirecciones. |
| H03 | Los recursos autenticados del Broker solo pueden descargarse desde su mismo origen. | Servidor simulado comprueba que otro origen nunca recibe `x-admin-token`. |
| H04 | La clasificación más restrictiva se hereda al reutilizar respuestas y resúmenes. | Turno posterior y generación de resumen mantienen `local_only`. |
| H05 | La decisión de cada herramienta se guarda antes del efecto y cada desenlace sustituye el estado pendiente. | Prueba de éxito y fallo durable sin confirmaciones colgadas. |
| H06 | El lector de HTML avanza por caracteres Unicode sin usar índices de bytes inválidos. | HTML con etiquetas y texto multibyte. |
| H07 | Al arrancar se recuperan claims programadas que quedaron persistidas antes del despacho. | Reinicio sintético recupera exactamente la claim huérfana. |
| H08 | Los estados terminales de workflows no se reabren por escrituras tardías y un fallo de cancelación remota se muestra. | Cancelación concurrente y rechazo remoto. |
| H09 | Una tarea local `orphaned` cierra como fallida su ejecución programada. | Reconciliación con estado remoto aún no terminal. |
| H10 | El sondeo de nodos reintenta transporte, 408, 429 y 5xx; al agotar reintentos abandona la tarea remota. | Recuperación tras 503 y cancelación tras agotamiento. |
| H11 | Publicación y ejecución usan un único presupuesto de memoria según el perfil del GPT. | Perfil Amplio resuelve 30 recuerdos publicados. |
| H12 | La versión publicada guarda huellas de memoria y revoca contenido editado hasta republicar. | Edición posterior deja de resolver la memoria congelada. |
| H13 | Se rechazan etiquetas de resultados duplicadas tras normalizar espacios y mayúsculas. | Validación evita la colisión del mapa de salidas. |
| H14 | SSE se reconstruye por bytes, acepta LF/CRLF/CR y conserva UTF-8 dividido entre lecturas, con límite de 1 MiB. | Evento fragmentado con Unicode y variantes de separador. |
| H15 | Las respuestas de investigación, artefactos y Markdown convertido se leen en streaming con límites duros. | Cuerpos por encima del límite se rechazan antes de acumularse. |
| H16 | Cada respuesta completada permite listar y guardar sus ficheros generados. | Prueba de componente recorre tarea local, artefacto remoto y guardado. |
| H17 | El SHA-256 anunciado se compara con los bytes descargados y una copia local alterada se repara. | Prueba de discrepancia y caché manipulada. |
| H18 | Una base con `user_version` futuro se rechaza antes de cambiar pragmas o migrar. | Esquema 999 conserva versión, tabla y contenido. |
| H19 | Los mensajes mayores que el lote se resumen mediante fragmentos Unicode trazables con cursor durable. | Dos lotes continúan en el carácter exacto y el contexto excluye lo ya cubierto. |
| H20 | Backend, formulario y agenda usan la zona IANA persistida y sus transiciones horarias. | `Atlantic/Canary` conserva las 10:00 al entrar y salir del horario de verano. |
| H21 | El lanzador compara una huella de nombres, tamaños y contenido de todos los insumos de Rust, Tauri y Vite. | Huella vigente devuelve 0; una huella alterada devuelve 1. |
| H22 | La cobertura incluye todo el frontend y CI construye MSI/NSIS, extrae el MSI y exige que contenga el ejecutable. | 319 pruebas frontend; línea base global publicada en `vite.config.ts`. |
| H23 | El contrato remoto `snake_case` se transforma a una vista IPC `camelCase`. | Serialización completa coincide con el tipo consumido por React. |

## Cambios de contrato y datos

- La versión del producto queda unificada en `0.2.0`.
- El esquema SQLite sube de 23 a 24 para guardar el cursor de fragmentos de resumen.
- Las bases de datos de versiones futuras se rechazan con un mensaje de actualización o
  restauración; no se intenta degradarlas.
- La tabla IANA embebida se genera con `scripts/generate_timezone_data.py` a partir de
  tzdata. Los datos de zonas horarias de IANA son de dominio público.
- `CHATYGPT_BROKER_BASE_URL`, `CHATYGPT_ATHENA_PREFERRED_MODEL` y
  `CHATYGPT_ATHENA_ALLOWED_MODELS` permiten adaptar el entorno sin editar el lanzador.

## Límites de la evidencia

La suite local valida código, contratos, persistencia y servidores simulados. CI queda
preparada para comprobar los instaladores en Windows. La firma del binario, una instalación
interactiva y la comparación visual de la ventana real siguen siendo evidencias de release,
no propiedades que puedan deducirse de una prueba unitaria.

## Validación final

- Rust: 338 pruebas aprobadas; formato y análisis estricto sin avisos.
- Frontend: 319 pruebas aprobadas; 56,20 % de líneas, 72,61 % de ramas y
  41,53 % de funciones cubiertas sobre todo el código de la interfaz.
- Contratos, migraciones y configuración: 43 pruebas aprobadas.
- Producción: TypeScript, Vite y Tauri aprobaron; se generó `chatygpt.exe` 0.2.0
  en modo optimizado y el lanzador reconoció su huella como vigente.
- Entrega: el YAML de CI es válido y queda configurado para construir MSI y NSIS y
  comprobar el contenido del MSI.
- Servicios reales: Agora estaba escuchando localmente en su puerto configurado. No se
  ejecutó una tarea real contra Broker AI porque el entorno de pruebas bloqueó el acceso
  a la dirección LAN antes de enviar la petición; esta limitación no afecta a las pruebas
  de contrato con servidores simulados.
