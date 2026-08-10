-- Fase 4: permite anidar Investigación profunda dentro del flujo semántico.
--
-- Hasta ahora, activar ambos controles hacía que la investigación tuviera
-- prioridad y la recuperación semántica se descartara en silencio. El motivo
-- real no era técnico sino de política: no existía una decisión durable sobre
-- qué debía ocurrir tras un reinicio con dos workflows anidados.
--
-- Esta columna es esa política. El plan de investigación —las herramientas que
-- el Broker anunciaba y se validaron al enviar— se congela aquí antes de
-- persistir nada. La segunda etapa y una recuperación posterior aplican
-- exactamente el mismo plan, sin volver a negociar capacidades y sin que un
-- Broker reiniciado con otras herramientas cambie una investigación en curso.
--
-- `NULL` significa turno semántico ordinario, que es el comportamiento de todas
-- las filas anteriores a esta migración.

ALTER TABLE semantic_chat_workflows
    ADD COLUMN research_plan_json TEXT;
