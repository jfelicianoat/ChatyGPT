# Consultas a APIs externas desde un GPT personal

ChatyGPT permite que un GPT personal proponga una consulta puntual a una API pública. La capacidad está denegada por defecto y cada llamada requiere aprobación explícita.

## Dónde encontrarla

1. Abre **Inicio**.
2. Entra en **GPTs personales**.
3. Crea o edita un GPT.
4. En **Permisos de herramientas**, activa **Consultar APIs externas**.
5. Guarda la nueva versión y abre un chat con ese GPT.

## Cómo probarla

Activa también **Herramientas** en el compositor y escribe una petición explícita que incluya la URL completa, por ejemplo:

> Consulta la API https://api.github.com/repos/tauri-apps/tauri y dime cuántas estrellas tiene.

ChatyGPT debe mostrar una confirmación con el dominio y la URL exacta. Si la rechazas, no se realiza ninguna conexión. Si la apruebas, la respuesta textual de la API vuelve al GPT para terminar el turno.

## Límites de seguridad

- Solo peticiones `HTTPS GET`.
- Sin credenciales, cuerpo ni cabeceras personalizadas.
- Se bloquean el propio equipo, la red local y las URLs con usuario o contraseña.
- No se siguen redirecciones.
- Tiempo, descarga y texto devuelto están acotados.
- Cada URL se confirma una sola vez; el permiso no equivale a aprobación permanente.
- Las versiones antiguas de un GPT mantienen esta capacidad denegada si no la tenían configurada.

Estos límites hacen que la primera versión sea adecuada para datos públicos de solo lectura. APIs autenticadas o con operaciones de escritura requieren un diseño separado de secretos, destinos autorizados y consecuencias.
