# Design QA — rediseño de usabilidad, dirección 2

## Referencia

- Imagen seleccionada: `C:\Users\jfeli\.codex\generated_images\019f7b9c-c5f7-7673-a71f-346b6aa7cc6d\exec-69907341-dc78-4d1e-882e-5352ccf05dc2.png`
- Estado objetivo: conversación abierta con navegación principal, mensajes, compositor y panel derecho de contexto activo.
- Viewport objetivo de la referencia: escritorio panorámico.

## Elementos implementados

- Navegación principal estable: Chats, Proyectos, GPTs, Automatizaciones y Ajustes.
- Acciones secundarias de conversación agrupadas en el menú Más.
- Columna de conversación con el compositor anclado en la parte inferior.
- Inspector lateral plegable de contexto activo.
- Desactivación de adjuntos para el próximo turno desde el inspector, sin borrar el archivo de la conversación.
- Resumen visible de proyecto, memoria y privacidad.
- Página específica de proyectos y separación visual de las áreas principales.
- Adaptación a tema oscuro y anchuras de escritorio reducidas.

## Verificaciones automáticas

- TypeScript: aprobado.
- Compilación de producción de Vite: aprobada.
- Suite completa: 156 pruebas aprobadas.
- Prueba específica de la navegación nueva: aprobada.
- Comprobación de espacios y errores de parche: aprobada.

## Comparación visual

No se pudo obtener una captura fiable de la aplicación ejecutándose en esta sesión. Tauri compiló, pero el entorno aislado impidió abrir su base de datos en el directorio local de Windows (`unable to open database file`), por lo que no llegó a crear una ventana capturable. Una captura fabricada o de un montaje distinto no sería una comprobación válida contra la referencia.

## Revisión manual pendiente

1. Reiniciar ChatyGPT con el BAT habitual.
2. Abrir una conversación que tenga al menos un adjunto y un proyecto.
3. Comparar a 1280 × 820 o superior con la referencia.
4. Confirmar que el panel Contexto activo no corta nombres y que el compositor permanece visible.
5. Reducir la ventana hasta su ancho mínimo y confirmar que la conversación sigue siendo utilizable.

## Resultado final

**BLOQUEADO para aprobación visual.** La implementación y sus pruebas técnicas están completas, pero falta una captura de la aplicación real para hacer la comparación lado a lado exigida por el proceso de diseño.
