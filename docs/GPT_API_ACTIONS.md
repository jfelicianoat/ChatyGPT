# Acciones API configuradas en GPTs personales

Una acción API permite enseñar a un GPT personal a consultar un destino público conocido sin dejar que el modelo invente la dirección. La acción queda guardada con la versión del GPT y cada ejecución requiere confirmación.

## Dónde encontrarla

1. Abre **Inicio → GPTs personales**.
2. Crea o edita un GPT.
3. En **Permisos de herramientas**, activa **Consultar APIs externas**.
4. En **Acciones API configuradas**, pulsa **Añadir acción API**.

Cada acción contiene:

- un nombre interno, por ejemplo `consultar_tiempo`;
- una descripción para que el GPT sepa cuándo usarla;
- una URL HTTPS fija, que puede contener variables de ruta como `{name}`;
- hasta ocho parámetros de ruta o consulta; cada uno puede ser texto, número o sí/no, obligatorio u opcional, y tener una explicación para el modelo. Los de ruta siempre son obligatorios.
- autenticación opcional mediante **Token Bearer** o cabecera **X-API-Key**.

Si la API necesita autenticación, elige el tipo, escribe un alias como
`mi_servicio`, pega la clave y pulsa **Guardar credencial**. ChatyGPT cifra el
secreto con la protección de tu cuenta de Windows. La versión del GPT guarda
solo el alias: la clave no entra en SQLite, no se exporta, no se muestra de
nuevo y nunca se entrega al modelo. Un mismo alias puede reutilizarse en varias
acciones. **Retirar del equipo** borra la clave y deja esas acciones inactivas
hasta que vuelvas a guardarla.

## Cómo probarla

Configura una API pública de prueba:

- Nombre: `buscar_pais`
- Descripción: `Busca datos públicos de un país por nombre`
- URL: `https://restcountries.com/v3.1/name/{name}`
- Parámetro: nombre `name`, tipo **Texto**, ubicación **Ruta**, obligatorio y descripción `Nombre del país`.

Guarda el GPT, abre un chat con él, activa **Herramientas** y pregunta:

> Busca los datos de España y dime su capital y población.

ChatyGPT debe mostrar una confirmación con `restcountries.com`, el parámetro que se enviará y el alcance **una sola vez**. Rechazarla impide la conexión; aprobarla devuelve la respuesta al GPT.

Antes de guardar puedes introducir valores de ejemplo y pulsar **Previsualizar**. ChatyGPT valida la plantilla y muestra la URL final, el destino y cuántos datos aparecerán en la confirmación. Esta previsualización es completamente local: no abre la API, no contacta con Broker AI y no genera coste. Si hay autenticación, solo se muestra el alias; nunca el valor protegido.

Pulsa **Probar conexión** si también quieres comprobar la respuesta real. Antes
de conectarse, ChatyGPT muestra el servidor, el número de parámetros y el alias
de la credencial que usará, y pide una
confirmación explícita. La prueba abre la API una sola vez, sin pasar por Broker
AI ni usar un modelo, y presenta el estado HTTP, el tiempo, el tipo de contenido
y una vista limitada de la respuesta. Cancelar la confirmación no realiza ninguna
conexión.

## Límites

- Máximo de 10 acciones por GPT y 8 parámetros por acción.
- Solo HTTPS GET público, sin cuerpos ni cabeceras arbitrarias. La autenticación se limita a `Authorization: Bearer` o `X-API-Key` con una credencial protegida.
- El modelo no puede cambiar la URL fija.
- Se bloquean red local, bucle local, direcciones privadas y redirecciones.
- La respuesta textual, el tiempo y el tamaño están limitados.
- Las acciones no se incluyen al exportar o duplicar un GPT, para no propagar accesos externos silenciosamente. Las credenciales tampoco abandonan este equipo.
