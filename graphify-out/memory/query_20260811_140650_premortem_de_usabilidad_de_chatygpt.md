---
type: "usability"
date: "2026-08-11T14:06:50.156723+00:00"
question: "Premortem de usabilidad de ChatyGPT"
contributor: "graphify"
outcome: "useful"
---

# Q: Premortem de usabilidad de ChatyGPT

## Answer

El mayor riesgo no es falta de funciones sino exceso de superficie simultánea: Inicio reúne estado local, Broker, apariencia, rendimiento, credencial, carpetas, programación, GPTs, memoria, prueba y actividad; el chat añade una barra de siete acciones, adjuntos, herramientas y opciones de ejecución. La mitigación prioritaria es separar trabajar/configurar/administrar, aplicar divulgación progresiva, consolidar estados y errores cerca de la acción, y validar tipografía, reflow y accesibilidad con una sesión visual interactiva. La arquitectura App.tsx de 7.372 líneas y 80 estados aumenta el riesgo de inconsistencias de interacción.

## Outcome

- Signal: useful