# Modificación segura de archivos por GPTs personales

Este corte permite que un GPT personal proponga reemplazar el contenido de un archivo de texto existente. No crea ni borra archivos.

## Activación

1. En **Ajustes > Carpetas autorizadas**, pulsa **Autorizar carpeta para modificar**.
2. En **GPTs**, crea o edita un GPT y activa **Modificar archivos autorizados**. La lectura se activa también porque es necesaria para comprobar la versión del archivo.
3. En un chat que use ese GPT, activa **Herramientas** y pide: `Edita el archivo notas.txt de la carpeta autorizada y corrige su redacción`.
4. Aprueba el listado, la lectura y, después de revisar el aviso, el reemplazo propuesto.

## Protecciones

- Cada operación exige confirmación individual.
- Solo se modifican archivos de texto existentes, UTF-8 y de hasta 256 KB.
- El GPT trabaja con rutas relativas y nunca recibe la ruta absoluta.
- La lectura devuelve una huella SHA-256. El reemplazo solo se realiza si esa huella sigue coincidiendo.
- Si el archivo cambió fuera de ChatyGPT, se rechaza el reemplazo y debe volver a leerse.
- La escritura usa un archivo temporal sincronizado y un reemplazo atómico.
- La tarea se restringe a modelos locales.
- La modificación queda registrada en auditoría mediante las huellas anterior y posterior, sin guardar el contenido ni la ruta.
