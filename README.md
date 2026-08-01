# ChatyGPT

Aplicación de escritorio Windows, local-first, para conversar mediante AI Broker
sin acoplar la interfaz a su API HTTP.

## Estado

Fase 4 en curso. La base durable de las fases anteriores, los GPTs personales y
el primer workflow de investigación incluyen:

- shell Tauri 2 + React + TypeScript;
- SQLite local con migración inicial y recuperación de tareas activas;
- adaptador tipado de AI Broker 2.7;
- descubrimiento automático al arrancar de salud, carriles, frontera de datos,
  sandbox, ingesta y soporte de documentos largos;
- recorrido durable opcional: persistir, enviar, sondear, cancelar y recuperar;
- conversaciones y mensajes persistentes con compositor multi-turno;
- snapshot trazable de la ventana de contexto utilizada por cada turno;
- materialización idempotente del resultado remoto como mensaje asistente;
- creación, renombrado, archivado y eliminación lógica de conversaciones;
- proyectos locales con asociación reversible de conversaciones;
- catálogo local de GPTs personales con nombre, descripción e instrucciones;
- versiones inmutables: cada edición crea una revisión numerada y conserva las anteriores;
- historial de versiones consultable y restauración de una revisión anterior, que
  crea una versión nueva sin borrar ninguna ni alterar las respuestas ya emitidas;
- duplicación de un GPT sin arrastrar permisos concedidos ni conocimiento privado;
- vista previa que muestra el bloque exacto que recibiría el modelo, los permisos
  vigentes y qué conocimiento o archivos no se usarían todavía, sin enviar nada
  a Broker AI ni generar coste;
- modelo preferido por GPT, enviado como preferencia con reserva automática, y
  proyecto predeterminado que solo alcanza a los chats aún sin clasificar;
- selección reversible de un GPT personal por conversación;
- captura por valor de la versión activa al enviar, incluso antes de una búsqueda
  semántica, con sus instrucciones visibles en el contexto exacto de la respuesta;
- matriz de permisos versionada para Código aislado y Renombrar conversación,
  denegada por defecto y siempre sujeta a confirmación;
- hasta seis iniciadores editables por versión, visibles como propuestas en los
  chats vacíos que usan ese GPT;
- exportación portable segura: configuración sola por defecto o, mediante una
  acción explícita, conocimiento textual activo y no sensible; nunca incluye
  IDs internos, permisos, archivos ni datos sensibles;
- importación compatible con ambos formatos, con permisos denegados y todo
  conocimiento recibido desactivado hasta que la persona lo revise;
- Investigación profunda activable por turno, ejecutada como agente multifuente
  con plan, búsquedas, lectura, síntesis y citas; su expediente y sus etapas
  persisten, se recuperan tras reiniciar y comparten la cancelación durable; los
  enlaces web reales del informe se guardan como fuentes deduplicadas y
  reaparecen al abrir o exportar la conversación;
- captura integrada de una pantalla o ventana, con selector seguro de Windows,
  previsualización, recorte visual local, compresión acotada y adjunto explícito
  mediante el flujo normal de ingesta;
- fotografía desde webcam iniciada por el usuario, sin audio ni grabación,
  con indicador de cámara activa, permiso revocable, vista previa y confirmación
  antes de incorporarla al chat;
- tareas locales únicas, diarias o semanales, vinculadas a una conversación y
  una zona horaria, con edición, confirmación explícita, pausa, claim key
  idempotente, historial durable y avisos de finalización cuando WebView2 los
  permite;
- conocimiento textual privado por GPT personal, utilizable incluso con la memoria
  general desactivada, indexable para búsqueda semántica y visible como fuente
  diferenciada en el contexto exacto de cada respuesta;
- hasta 20 archivos de conocimiento privados por GPT, ingeridos e indexados por
  la ruta documental común y resueltos dinámicamente al enviar, sin crear enlaces
  residuales en la conversación al cambiar o retirar el GPT;
- instrucciones persistentes por proyecto, aplicadas a todos sus chats y
  conservadas como fuente visible en el contexto exacto de cada respuesta;
- vista unificada del conocimiento de cada proyecto con instrucciones, archivos
  reutilizables, recuerdos limitados al proyecto y sus estados reales, con
  accesos a los chats que usan cada archivo y controles auditados para retirar
  archivos y activar o desactivar recuerdos, además de búsqueda sin distinción
  de acentos y filtros por tipo;
- biblioteca explícita de archivos por proyecto, reutilizable entre sus chats sin
  nuevas subidas ni inyección automática de contexto;
- búsqueda por título y contenido de mensajes;
- confirmaciones explícitas y auditoría para operaciones de ciclo de vida;
- recuperación visual de una tarea pendiente al reabrir su conversación;
- adjuntos reutilizables con copia local administrada, SHA-256 y deduplicación;
- selección nativa y arrastre de archivos, subida en streaming y seguimiento de ingesta;
- envío al chat solo cuando Broker AI confirma el `file_id` como `ready`;
- fuentes documentales trazables bajo cada respuesta, derivadas de los adjuntos enviados;
- modo de herramientas opt-in con confirmación individual antes de cualquier acción local;
- primera herramienta de cliente: renombrar la conversación solo tras autorización visible;
- decisiones y resultados de herramientas persistidos antes de reanudar AI Broker;
- exportación Markdown mediante diálogo nativo, con fuentes documentales y sin rutas internas;
- escritura atómica, comprobación SHA-256, detección de cambios externos y auditoría del exportado;
- exportación directa a una bóveda de Obsidian como proyección estructurada, con YAML,
  enlaces estables a proyecto y fuentes, y copia verificada de adjuntos;
- índices de proyecto y memoria aprobada regenerados al exportar a Obsidian, excluyendo
  recuerdos sensibles y protegiendo cualquier edición externa;
- ejecución opcional de Python en el sandbox desechable de Broker AI, habilitada para un solo turno;
- comprobación redundante de la capacidad `sandbox_run_code` antes de persistir y enviar la tarea;
- aviso explícito cuando el mensaje pide ejecutar o probar código sin haber concedido todavía el permiso;
- privacidad, estrategia, profundidad, coste máximo y tratamiento de documentos largos configurables por conversación;
- selección entre modelos locales y proveedores cloud habilitados, gobernada únicamente por la clasificación de datos del contrato 2.7;
- presupuesto duro por petición (0, 0,10, 0,50 o 1 USD) y protección automática que conserva en local los recuerdos sensibles;
- respuesta directa, decisión automática del Broker o análisis en equipo, limitados a las capacidades y presets que anuncia el Broker;
- map-reduce explícito para adjuntos que no caben, sin truncado silencioso;
- progreso por fases e invocaciones y errores remotos traducidos a mensajes legibles con indicación de reintento;
- proveedor y modelo utilizados visibles debajo de cada respuesta;
- tiempo total de respuesta visible y durable, conservado al reabrir la conversación;
- respuestas del asistente presentadas como texto enriquecido seguro, con títulos,
  listas, tablas, citas, enlaces y bloques de código, conservando el Markdown
  original para historial, contexto y exportación;
- inspector de actividad reciente con descripciones legibles y severidad visual;
- proyección segura de auditoría que excluye prompts, tokens, rutas, hashes y JSON técnico;
- aviso global de recuperación al reiniciar, con recuento de tareas y adjuntos reanudados;
- acceso directo desde el aviso a cada conversación recuperada;
- memoria personal desactivada por defecto y activable explícitamente desde Inicio;
- recuerdos manuales globales o limitados a un proyecto, con edición inline, control individual y borrado;
- corrección segura de contenido, categoría, sensibilidad y ámbito, con reindexación automática solo cuando cambia el texto;
- protección contra resultados tardíos que intenten guardar el embedding de una versión anterior del recuerdo;
- inclusión trazable de los recuerdos aprobados en el snapshot exacto de cada turno;
- inspector desplegable bajo cada respuesta con la estrategia, tamaño y fuentes del contexto realmente enviado;
- acceso desde cada fragmento documental del inspector a **Mostrar archivo**, que selecciona en Windows la copia local administrada sin exponer su ruta ni ejecutarla;
- estado explícito cuando la copia local de una fuente ya no está disponible;
- indexación durable de recuerdos mediante embeddings locales de AI Broker;
- estado visible `Indexando`, `Índice preparado`, `Sin índice` o `Error de índice`, con reintento manual;
- probador de búsqueda semántica local con ámbito, puntuación, motivo y recuerdo original visibles;
- consultas semánticas durables que se recuperan tras reiniciar y solo comparan vectores compatibles;
- selección semántica opcional en cada envío mediante **Buscar recuerdos**, con progreso visible;
- flujo durable de dos etapas que persiste el turno antes de vectorizar y conserva la selección en el inspector;
- resúmenes de conversación generados como borradores durables, editables y nunca activados automáticamente;
- aprobación explícita del resumen antes de compactar el contexto, sin borrar ni modificar el historial original;
- resumen aprobado visible y explicado en el inspector exacto de contexto de cada respuesta;
- actualización incremental del resumen en lotes seguros de hasta 48.000 caracteres, reutilizando el resumen aprobado y sin reenviar todo el historial;
- cobertura visible de mensajes resumidos y pendientes, con los mensajes recientes fuera del lote conservados en la ventana normal;
- registro estructurado local con correlación por tarea, incapaz por construcción
  de contener prompts, rutas ni secretos, con rotación acotada;
- expediente durable de cada confirmación de herramienta, con acción, recursos,
  datos, destino, alcance y consecuencias, resuelto antes de ejecutar y sin
  posibilidad de repetirse;
- carpetas autorizadas revisables y revocables desde Inicio: ChatyGPT solo
  escribe donde la persona eligió en un selector de Windows;
- credencial de Broker AI cifrada con DPAPI para la cuenta de Windows, con alta
  y retirada desde la aplicación y sustitución en caliente;
- fixture contractual local-only y sin coste cloud;
- pruebas ejecutables con la biblioteca estándar de Python.

El recorrido normal de conversación sigue
`persistir turno y contexto → crear tarea → sondear → materializar respuesta`.
Con **Buscar recuerdos** sigue
`persistir turno → vectorizar consulta → seleccionar recuerdos compatibles → crear tarea de chat → materializar respuesta`;
ambas etapas se recuperan después de reiniciar.
Los adjuntos que el Broker convierte a Markdown se dividen además en fragmentos
locales de hasta 4.000 caracteres, priorizando finales de párrafo y frase para no
cortar el contenido arbitrariamente. En cada turno se seleccionan como máximo ocho
fragmentos relacionados y su contexto próximo, con un presupuesto total de 24.000 caracteres; cuando
hay selección local, el archivo completo no se vuelve a enviar al modelo. La
respuesta permite revisar cada fragmento desde **Ver contexto utilizado**.
Cada fragmento se indexa además de forma progresiva mediante embeddings locales:
solo hay una tarea activa por documento, el trabajo continúa tras reiniciar y la
recuperación combina significado y coincidencias literales. Si el modelo de la
consulta no es compatible o falla un vector, se conserva la selección por texto.
Cuando la conversión falla, la tarjeta del adjunto traduce los límites conocidos
del Broker a una explicación accionable. En un PDF que excede el máximo de
páginas muestra las páginas reales, el límite admitido y las alternativas antes
de ofrecer **Reintentar tras corregir**.
La subida y la preparación del contexto se controlan por separado: un archivo
puede estar disponible en Broker aunque todavía no tenga fragmentos locales. Su
tarjeta muestra el progreso, el número de fragmentos, los caracteres consultables,
una estimación de tokens y el avance del índice semántico. **Reintentar contexto**
no vuelve a subir el documento y **Reintentar índice** recupera los fragmentos
semánticos fallidos.
Los resúmenes se gestionan desde **Resumen** en la cabecera de cada conversación:
`generar borrador → revisar o editar → guardar y aprobar`. Hasta el último paso,
el resumen no entra en ninguna petición. Si quedan mensajes sin cubrir, el panel
muestra el recuento y ofrece **Actualizar borrador** para avanzar otro lote.
La petición HTTP se realiza en segundo plano después del commit local y se
reintenta con la misma clave idempotente ante errores transitorios.
Las automatizaciones se gestionan desde **Inicio → Tareas programadas**. Admiten
una ejecución, repetición diaria o semanal, edición, pausa y un historial durable.
Una ejecución fallida puede reintentarse como un intento nuevo sin borrar el
fallo anterior; una ejecución activa puede cancelarse sin pausar la siguiente
repetición. El historial se filtra por estado y fecha, muestra el resultado o
motivo de fallo como texto legible y puede exportarse a un `.txt` verificado
aplicando esos mismos filtros. El botón **Avisos** reúne las finalizaciones
recientes dentro de ChatyGPT, incluso cuando los avisos de Windows no están
disponibles. El buscador localiza programaciones por nombre, conversación o
texto de la instrucción sin depender de mayúsculas ni acentos. Las plantillas
reutilizables conservan solo nombre, instrucción y repetición: al usarlas siguen
siendo obligatorios elegir o revisar conversación y fecha, y confirmar de nuevo
antes de activar la tarea.
Cada tarjeta permite además **Duplicar** la programación como un borrador sin
confirmar y **Ejecutar ahora** con confirmación. La ejecución manual se añade al
historial, impide solapamientos y no pausa ni desplaza la siguiente repetición.
**Ver historial completo** abre una consulta paginada de 10, 25 o 50 ejecuciones,
permite ordenar desde la más reciente o la más antigua y respeta los filtros de
estado y fecha sin limitarse al resumen reciente de la tarjeta.
**Calendario** presenta las próximas fechas en una agenda de 7, 14 o 30 días.
La primera fecha de cada tarea procede del registro durable; las recurrencias
posteriores se identifican como proyecciones de solo lectura. Las tareas
atrasadas aparecen separadas y se avisa cuando dos programaciones distintas
quedan a 15 minutos o menos, con acceso directo a la conversación de destino.
Desde esa misma vista, **Exportar .ics** guarda exactamente las fechas visibles
para abrirlas en Outlook, Calendario de Windows u otra aplicación compatible.
El archivo solo incluye nombre de tarea, conversación, fecha y clase de fecha;
nunca incluye la instrucción, el resultado ni el contexto del chat.
El bloque **Inicio con Windows** permite que estas automatizaciones sigan
funcionando tras iniciar sesión sin abrir la aplicación a mano. Es una opción
reversible por usuario: no instala servicios ni solicita permisos de
administrador. La credencial del Broker se cifra con DPAPI para la cuenta de
Windows, no se guarda en React, SQLite ni en el script de arranque. El iniciador
espera a que el Broker acepte una consulta autenticada de capacidades y evita
abrir una segunda instancia. Si cambia el token, basta con abrir una vez
ChatyGPT mediante `Arrancar ChatyGPT.bat` para renovar la copia protegida.
En **Inicio → Apariencia** puede elegirse tema **Windows**, **Claro** u
**Oscuro**. La preferencia se guarda solo en WebView2 y se aplica en el HTML
antes de cargar React, evitando el destello de un tema distinto al abrir la
aplicación. En modo Windows, un cambio del tema del sistema se refleja mientras
ChatyGPT está abierto.
La barra lateral incorpora **Atajos de teclado**. Además de mostrar la ayuda,
ChatyGPT permite crear un chat con `Ctrl+N`, buscar con `Ctrl+F` o `/`, llevar el
foco al compositor con `Ctrl+Mayús+M`, volver a Inicio con `Alt+1` y abrir la
propia ayuda con `?`. Las teclas simples no se capturan mientras se escribe. Al
recorrer la página con Tab aparece **Saltar al contenido principal**; las
ventanas mantienen el foco dentro, admiten `Esc` y lo devuelven al control que
las abrió.

## Desarrollo

El entorno Windows auditado ya dispone de Rust estable y de las dependencias
JavaScript. Se han verificado TypeScript, Vite, Cargo y una construcción Tauri
de producción. Para desarrollo:

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

Verificación contractual contra una instancia real:

```powershell
python scripts\verify_broker.py --base-url http://127.0.0.1:8765
python scripts\verify_broker.py --base-url http://127.0.0.1:8765 --smoke-task
```

El segundo comando crea una tarea `single`, `local_only`, con proveedores cloud
deshabilitados y coste máximo cero. Repite el mismo POST para comprobar
idempotencia y sondea hasta estado terminal.

Configuración no secreta:

- `CHATYGPT_BROKER_BASE_URL`, por defecto `http://192.168.1.52:8765` para evitar
  que el ejecutable publicado intente usar por error el loopback de este equipo.

Para la instancia personal verificada en `A9_Mega`, antes de iniciar Tauri:

```powershell
$env:CHATYGPT_BROKER_BASE_URL = "http://192.168.1.52:8765"
```

Dentro de la app, primero se usa **Comprobar conexión**. Cuando Broker AI está
listo, se puede crear una conversación y enviar el primer mensaje.

En cada conversación, **Opciones de ejecución** aparece debajo del cuadro de
mensaje. Sus valores se guardan con ese chat y se aplican también cuando un
envío con búsqueda semántica continúa después de reiniciar. **Uso personal**
permite al Broker elegir tanto modelos locales como cloud; **Confidencial** y
**Solo en este equipo** impiden que el contenido salga a proveedores externos.
**Análisis en equipo** habilita la profundidad normal o exhaustiva únicamente
si el Broker anuncia esos presets. **Dividir documentos que no caben** autoriza
map-reduce solo para estrategias compatibles.

Una conversación se guarda desde **Exportar Markdown**. Si el archivo ya
existe, Windows solicita confirmación antes de reemplazarlo. ChatyGPT escribe
el resultado de forma atómica y verifica su huella antes de declararlo
completado.

La acción **Obsidian**, situada junto a **Markdown** en la cabecera de una
conversación, permite elegir una bóveda o carpeta. ChatyGPT crea dentro de ella
`ChatyGPT/Conversaciones`, `ChatyGPT/Proyectos` y `ChatyGPT/Adjuntos`. Las notas
incluyen metadatos YAML e identificadores estables; los adjuntos se reutilizan
cuando su SHA-256 coincide. Si una nota o copia fue modificada fuera de la app,
se pide confirmación antes de reemplazarla. SQLite continúa siendo la fuente de
verdad y nunca se copia a la bóveda.

La opción **Código aislado · un turno** solo se habilita cuando Broker AI
publica la capacidad `sandbox_run_code`. El permiso se consume tras el siguiente
envío; la ejecución sucede en el contenedor restringido del Broker, nunca en el
proceso de ChatyGPT ni con acceso a los archivos del equipo. Las herramientas
de interfaz cambian temporalmente ese envío al modo agente. Si la conversación
usa **Análisis en equipo**, Código aislado se entrega a sus proponentes mediante
`run_code`, sin perder la estrategia colaborativa guardada.

Credencial de Broker AI:

- Se guarda desde **Inicio → Credencial de Broker AI**, cifrada con DPAPI para
  la cuenta de Windows que la introduce. No se persiste en SQLite, ni en los
  registros, ni en el script de arranque, y no vuelve a mostrarse.
- `AI_BROKER_ADMIN_TOKEN` sigue admitiéndose como vía de transición: solo se usa
  cuando no hay credencial guardada. La app prefiere siempre el almacén cifrado.
- Guardar una credencial nueva la aplica sin reiniciar y actualiza la copia
  protegida del inicio con Windows si estaba activo.
- `Arrancar ChatyGPT.bat` reutiliza la credencial guardada y solo pide el token
  cuando no existe o no puede descifrarse.

## Documentación

- [Arquitectura y plan](docs/ARCHITECTURE.md)
- [Endurecimiento de Fase 0](docs/PHASE_0_HARDENING.md)
- [Cierre de huecos de Fase 3](docs/PHASE_3_COMPLETION.md)
- [Evidencias de Fase 0](docs/PHASE_0_VERIFICATION.md)
- [Evidencias de Fase 1](docs/PHASE_1_VERIFICATION.md)
- [Evidencias de Fase 2](docs/PHASE_2_VERIFICATION.md)
- [Contrato local AI Broker 2.7](contracts/broker/2.7/single-task.request.json)
