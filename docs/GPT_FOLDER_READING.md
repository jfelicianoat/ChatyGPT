# Lectura segura de carpetas por GPTs personales

ChatyGPT separa la autorización de una carpeta y el permiso de cada GPT.

## Activación

1. En **Ajustes > Carpetas autorizadas**, pulsa **Autorizar carpeta para lectura** y elige una carpeta.
2. En **GPTs**, crea o edita un GPT y activa **Leer carpetas autorizadas**.
3. En un chat que use ese GPT, activa **Herramientas** y pide, por ejemplo: `Lista los archivos de la carpeta autorizada`.

Cada listado y cada lectura muestra una confirmación antes de ejecutarse. La petición se restringe a modelos locales.

## Límites de seguridad

- El modelo no recibe rutas absolutas.
- Solo se aceptan rutas relativas que permanezcan dentro de la carpeta autorizada.
- Los listados muestran un máximo de 100 elementos por llamada.
- Solo se leen formatos de texto y código, en UTF-8, hasta 256 KB.
- No se siguen rutas ni enlaces que salgan de la carpeta concedida.
- Revocar la carpeta en Ajustes impide las lecturas posteriores.
