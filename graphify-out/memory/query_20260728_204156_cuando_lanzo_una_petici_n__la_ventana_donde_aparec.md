---
type: "debugging"
date: "2026-07-28T20:41:56.523557+00:00"
question: "Cuando lanzo una petici?n, la ventana donde aparece la respuesta no hace scroll para que se vea esa ultima respuesta, siempre se queda en la pregunta o instruccion lanzada, y tengo que mover a mano el scroll para ver la respuesta. Esto no puede ser as?. Otra cosa es que vaya apareciendo una respuesta y me interese ver algo que est? mas arriba y me muevo hasta ese punto, en ese caso el scroll no debe ser automatico y volver a la ultima respuesta , sino quedarse donde lo he puesto"
contributor: "graphify"
outcome: "useful"
source_nodes: ["App()", "domain.ts", "domain.test.ts", "isTaskPollingComplete()", "shouldFollowConversationScroll()"]
---

# Q: Cuando lanzo una petici?n, la ventana donde aparece la respuesta no hace scroll para que se vea esa ultima respuesta, siempre se queda en la pregunta o instruccion lanzada, y tengo que mover a mano el scroll para ver la respuesta. Esto no puede ser as?. Otra cosa es que vaya apareciendo una respuesta y me interese ver algo que est? mas arriba y me muevo hasta ese punto, en ese caso el scroll no debe ser automatico y volver a la ultima respuesta , sino quedarse donde lo he puesto

## Answer

Expansi?n desde el vocabulario del grafo: [conversation, messages, response, polling, render, ref, turn, view, app, domain]. App() renderizaba .message-list con overflow, pero no ten?a una referencia ni un efecto que siguiera las actualizaciones de conversaci?n. Se a?adi? seguimiento inteligente mediante useLayoutEffect y shouldFollowConversationScroll: enviar reactiva el seguimiento, las actualizaciones desplazan al final solo mientras el lector est? a 96 px o menos del final, y un desplazamiento manual hacia arriba lo suspende. El sondeo final cambia una se?al derivada de mensajes/progreso, de modo que la respuesta queda visible sin secuestrar la posici?n del lector. Cubierto por dos regresiones TypeScript y revisi?n Impeccable sin hallazgos.

## Outcome

- Signal: useful

## Source Nodes

- App()
- domain.ts
- domain.test.ts
- isTaskPollingComplete()
- shouldFollowConversationScroll()