//! Pruebas de la proyección del área de Athena.
//!
//! Lo que se comprueba aquí no es que los campos se rellenen, sino que se
//! rellenan **solo** con lo que Athena publicó: la interfaz no deduce estado.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::contracts::{EventoRuntime, InstantaneaRun, MarcoEstado, MensajeFlujo};
use super::supervisor::{EstadoTarea, FaseRun, ProyeccionRun};

fn evento(nombre: &str, correlacion: Option<&str>, carga: Value) -> MensajeFlujo {
    let payload: BTreeMap<String, Value> = carga
        .as_object()
        .map(|mapa| mapa.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();
    MensajeFlujo::Evento(Box::new(EventoRuntime {
        event_id: format!("ev-{nombre}"),
        name: nombre.to_owned(),
        run_id: "run-1".to_owned(),
        correlation_id: correlacion.map(str::to_owned),
        occurred_at: "2026-08-19T00:00:00+00:00".to_owned(),
        payload,
    }))
}

fn instantanea(json_valor: Value) -> InstantaneaRun {
    serde_json::from_value(json_valor).expect("instantánea válida")
}

fn marco_estado(suscriptor: &str, controla: bool, snapshot: Option<Value>) -> MensajeFlujo {
    let valor = json!({
        "subscriber_id": suscriptor,
        "controls": controla,
        "wire_version": 1,
        "snapshot": snapshot,
        "pending_approvals": [],
    });
    let marco: MarcoEstado = serde_json::from_value(valor).expect("marco válido");
    MensajeFlujo::Estado(Box::new(marco))
}

fn base() -> ProyeccionRun {
    ProyeccionRun::nueva("run-1", "Arreglar calc.add", "D:/repo")
}

fn snapshot_json(estado: &str) -> Value {
    json!({
        "run_id": "run-1",
        "workspace_id": "ws-1",
        "status": estado,
        "resumable": estado == "recovery_pending",
        "degraded": false,
        "objective": "Arreglar calc.add",
        "created_at": "2026-08-19T00:00:00+00:00",
        "updated_at": "2026-08-19T00:00:01+00:00",
        "working_memory": {"objective": "Arreglar calc.add", "files_modified": ["calc.py"]},
        "verification": {"status": "passed", "summary": "Todo pasa"},
        "tool_references": [{"uri": "athena-result://k1", "store_key": "k1",
                             "media_type": "text/plain", "size_chars": 40000}],
        "checkpoints": [],
    })
}

// -- fases ----------------------------------------------------------------

#[test]
fn un_run_empieza_en_starting_y_solo_athena_lo_mueve() {
    let mut vista = base();

    assert_eq!(vista.fase, Some(FaseRun::Starting));

    vista.aplicar(&evento("agent.started", None, json!({})));
    assert_eq!(vista.fase, Some(FaseRun::Running));
}

#[test]
fn las_ocho_fases_provienen_del_runtime() {
    for (estado, esperada) in [
        ("running", FaseRun::Running),
        ("waiting_permission", FaseRun::WaitingPermission),
        ("verifying", FaseRun::Verifying),
        ("completed", FaseRun::Completed),
        ("failed", FaseRun::Failed),
        ("cancelled", FaseRun::Cancelled),
        ("recovery_pending", FaseRun::RecoveryPending),
    ] {
        let mut vista = base();
        vista.adoptar_instantanea(&instantanea(snapshot_json(estado)));
        assert_eq!(vista.fase, Some(esperada), "estado {estado}");
    }
}

#[test]
fn un_estado_que_no_conocemos_no_inventa_una_fase() {
    let mut vista = base();
    vista.aplicar(&evento("agent.started", None, json!({})));

    vista.adoptar_instantanea(&instantanea(snapshot_json("un_estado_del_futuro")));

    // Se conserva la última fase conocida en lugar de fabricar una.
    assert_eq!(vista.fase, Some(FaseRun::Running));
}

#[test]
fn un_run_por_recuperar_no_se_muestra_como_terminado() {
    let mut vista = base();

    vista.adoptar_instantanea(&instantanea(snapshot_json("recovery_pending")));

    assert_eq!(vista.fase, Some(FaseRun::RecoveryPending));
    assert!(!vista.fase.unwrap().es_terminal());
    assert!(vista.reanudable);
}

// -- instantánea frente a lo acumulado ------------------------------------

#[test]
fn una_instantanea_sustituye_lo_derivado_en_lugar_de_mezclarse() {
    let mut vista = base();
    vista.aplicar(&evento(
        "file.changed",
        None,
        json!({"path": "fantasma.py"}),
    ));
    assert_eq!(vista.ficheros_modificados, vec!["fantasma.py".to_owned()]);

    // Athena dice que el único fichero tocado es calc.py: gana Athena.
    vista.adoptar_instantanea(&instantanea(snapshot_json("running")));

    assert_eq!(vista.ficheros_modificados, vec!["calc.py".to_owned()]);
}

#[test]
fn el_marco_de_estado_entrega_la_identidad_para_poder_aprobar() {
    let mut vista = base();

    vista.aplicar(&marco_estado("sus-1", true, Some(snapshot_json("running"))));

    assert_eq!(vista.suscriptor.as_deref(), Some("sus-1"));
    assert!(vista.controla);
    assert!(vista.conectado);
}

// -- herramientas ---------------------------------------------------------

#[test]
fn una_herramienta_pasa_de_en_curso_a_terminada() {
    let mut vista = base();

    vista.aplicar(&evento(
        "tool.started",
        Some("c1"),
        json!({"tool_name": "read_file"}),
    ));
    assert_eq!(vista.herramientas[0].estado, "en curso");

    vista.aplicar(&evento(
        "tool.completed",
        Some("c1"),
        json!({"tool_name": "read_file", "externalized": true}),
    ));

    assert_eq!(vista.herramientas.len(), 1);
    assert_eq!(vista.herramientas[0].estado, "terminada");
    assert!(vista.herramientas[0].externalizado);
}

#[test]
fn una_herramienta_fallida_deja_su_error_a_la_vista() {
    let mut vista = base();
    vista.aplicar(&evento(
        "tool.started",
        Some("c1"),
        json!({"tool_name": "write_file"}),
    ));

    vista.aplicar(&evento(
        "tool.failed",
        Some("c1"),
        json!({"tool_name": "write_file", "error_code": "permission_denied",
               "message": "denegado"}),
    ));

    assert_eq!(vista.herramientas[0].estado, "fallida");
    assert_eq!(vista.errores[0].codigo, "permission_denied");
}

#[test]
fn la_accion_de_recuperacion_se_anota_en_el_error_que_la_provoco() {
    let mut vista = base();
    vista.aplicar(&evento(
        "tool.failed",
        Some("c1"),
        json!({"error_code": "tool_validation_error", "message": "mal"}),
    ));

    vista.aplicar(&evento(
        "recovery.action",
        Some("c1"),
        json!({"action": "inform_model"}),
    ));

    assert_eq!(
        vista.errores[0].recuperacion.as_deref(),
        Some("inform_model")
    );
}

// -- permisos -------------------------------------------------------------

#[test]
fn resolver_el_permiso_lo_retira_y_devuelve_la_fase() {
    let mut vista = base();
    vista.aplicar(&evento(
        "permission.requested",
        Some("req-1"),
        json!({"request_id": "req-1", "tool_name": "edit_file", "awaiting_decision": true}),
    ));

    vista.aplicar(&evento(
        "permission.resolved",
        Some("req-1"),
        json!({"decision": "allow", "asked": true}),
    ));

    assert!(vista.permisos.is_empty());
    assert_eq!(vista.fase, Some(FaseRun::Running));
}

#[test]
fn resolver_un_permiso_no_retira_los_demas() {
    let mut vista = base();
    for identificador in ["req-1", "req-2"] {
        vista.aplicar(&evento(
            "permission.requested",
            Some(identificador),
            json!({"request_id": identificador, "tool_name": "edit_file",
                   "awaiting_decision": true}),
        ));
    }

    vista.aplicar(&evento(
        "permission.resolved",
        Some("req-1"),
        json!({"decision": "deny"}),
    ));

    assert_eq!(vista.permisos.len(), 1);
    assert_eq!(vista.permisos[0].request_id, "req-2");
    assert_eq!(vista.fase, Some(FaseRun::WaitingPermission));
}

// -- verificación y evidencia ---------------------------------------------

#[test]
fn las_comprobaciones_se_emparejan_con_su_resultado() {
    let mut vista = base();

    vista.aplicar(&evento("verification.started", None, json!({})));
    vista.aplicar(&evento(
        "verification.check.started",
        None,
        json!({"check": "pytest -q"}),
    ));
    vista.aplicar(&evento(
        "verification.check.completed",
        None,
        json!({"check": "pytest -q", "passed": true}),
    ));

    assert_eq!(vista.fase, Some(FaseRun::Verifying));
    assert_eq!(vista.comprobaciones.len(), 1);
    assert_eq!(vista.comprobaciones[0].paso, Some(true));
}

#[test]
fn terminar_deja_la_evidencia_que_lo_justifica() {
    let mut vista = base();

    vista.aplicar(&evento(
        "agent.completed",
        None,
        json!({"iterations": 3, "repair_cycles": 1,
               "verification": "All project checks pass."}),
    ));

    assert_eq!(vista.fase, Some(FaseRun::Completed));
    assert_eq!(vista.ciclos_reparacion, 1);
    assert_eq!(vista.evidencia, vec!["All project checks pass.".to_owned()]);
}

#[test]
fn una_verificacion_fallida_se_explica_sin_adornos() {
    let mut vista = base();

    vista.aplicar(&evento(
        "verification.failed",
        None,
        json!({"reason": "introduced_failure", "checks": ["pytest"]}),
    ));

    assert_eq!(vista.verificacion.as_deref(), Some("failed"));
    assert!(vista
        .actividad
        .iter()
        .any(|linea| linea.contains("introduced_failure")));
}

// -- tareas y subagentes --------------------------------------------------

#[test]
fn el_plan_de_la_memoria_de_trabajo_se_muestra_como_tareas() {
    let mut vista = base();
    let mut snapshot = snapshot_json("running");
    snapshot["working_memory"] = json!({
        "objective": "Arreglar calc.add",
        "files_modified": [],
        "current_plan": [
            {"description": "Leer calc.py", "status": "done"},
            {"description": "Corregir el operador", "status": "in_progress"},
            {"description": "Ejecutar la suite", "status": "pending"}
        ],
        "current_step": 1
    });

    vista.adoptar_instantanea(&instantanea(snapshot));

    assert_eq!(vista.tareas.len(), 3);
    assert_eq!(vista.tareas[0].estado, EstadoTarea::Completed);
    assert_eq!(vista.tareas[1].estado, EstadoTarea::Running);
    assert_eq!(vista.tareas[2].estado, EstadoTarea::Pending);
}

#[test]
fn los_subagentes_aparecen_con_su_estado() {
    let mut vista = base();

    vista.aplicar(&evento(
        "subagent.started",
        Some("sub-1"),
        json!({"role": "explorer", "max_iterations": 8}),
    ));
    assert_eq!(vista.tareas[0].estado, EstadoTarea::Running);

    vista.aplicar(&evento(
        "subagent.completed",
        Some("sub-1"),
        json!({"role": "explorer", "status": "completed", "tool_calls": 3}),
    ));

    assert_eq!(vista.tareas.len(), 1, "el subagente no se duplica");
    assert_eq!(vista.tareas[0].estado, EstadoTarea::Completed);
    assert_eq!(vista.tareas[0].nombre, "explorer");
}

#[test]
fn los_siete_estados_de_tarea_se_reconocen() {
    for (texto, esperado) in [
        ("pending", EstadoTarea::Pending),
        ("running", EstadoTarea::Running),
        ("completed", EstadoTarea::Completed),
        ("failed", EstadoTarea::Failed),
        ("cancelled", EstadoTarea::Cancelled),
        ("killed", EstadoTarea::Killed),
        ("recovery_pending", EstadoTarea::RecoveryPending),
    ] {
        assert_eq!(super::supervisor::estado_tarea_desde(texto), Some(esperado));
    }
    assert_eq!(super::supervisor::estado_tarea_desde("inventado"), None);
}

// -- artefactos y actividad -----------------------------------------------

#[test]
fn los_artefactos_llegan_con_su_referencia_y_tamano() {
    let mut vista = base();

    vista.adoptar_instantanea(&instantanea(snapshot_json("completed")));

    assert_eq!(vista.artefactos.len(), 1);
    assert_eq!(vista.artefactos[0].clave, "k1");
    assert_eq!(vista.artefactos[0].tamano, 40000);
    assert!(vista.artefactos[0].uri.starts_with("athena-result://"));
}

#[test]
fn la_actividad_son_frases_operativas_y_no_se_repiten() {
    let mut vista = base();

    vista.aplicar(&evento("agent.started", None, json!({})));
    vista.aplicar(&evento(
        "tool.started",
        Some("c1"),
        json!({"tool_name": "grep"}),
    ));
    vista.aplicar(&evento(
        "tool.started",
        Some("c2"),
        json!({"tool_name": "grep"}),
    ));

    assert_eq!(
        vista.actividad,
        vec!["El agente ha empezado".to_owned(), "Usando grep".to_owned(),],
        "una línea idéntica seguida no se duplica"
    );
}

#[test]
fn la_proyeccion_no_expone_contenido_del_modelo() {
    // Los eventos de Athena no llevan el texto del asistente; esto fija que si
    // algún día lo llevaran, esta capa no lo arrastraría a la interfaz.
    let mut vista = base();
    vista.aplicar(&evento(
        "model.completed",
        None,
        json!({"finish_reason": "stop", "tool_call_count": 1,
               "content": "razonamiento interno que no debe verse"}),
    ));

    let serializado = serde_json::to_string(&vista).expect("serializable");
    assert!(!serializado.contains("razonamiento interno"));
}

#[test]
fn un_evento_desconocido_se_ignora_sin_romper_nada() {
    let mut vista = base();
    vista.aplicar(&evento("agent.started", None, json!({})));

    vista.aplicar(&evento("algo.que.no.existe", None, json!({"x": 1})));

    assert_eq!(vista.fase, Some(FaseRun::Running));
    assert_eq!(vista.actividad.len(), 1);
}

#[test]
fn el_contexto_compactado_y_la_reanudacion_se_explican() {
    let mut vista = base();

    vista.aplicar(&evento(
        "context.compacted",
        None,
        json!({"messages_before": 40, "messages_after": 6}),
    ));
    vista.aplicar(&evento("session.resumed", None, json!({"degraded": false})));

    assert!(vista
        .actividad
        .iter()
        .any(|l| l.contains("Contexto compactado")));
    assert!(vista.actividad.iter().any(|l| l.contains("reanudada")));
}

fn permiso(carga: Value) -> MensajeFlujo {
    evento("permission.requested", Some("req-1"), carga)
}

fn carga_permiso() -> Value {
    json!({
        "request_id": "req-1",
        "tool_name": "write_file",
        "operation": "write",
        "action": "escribir src/main.rs",
        "risk": "medium",
        "tier": "R2",
        "reason": "aplicar el cambio pedido",
        "possible_effects": ["modifica un fichero del repositorio"],
        "resources": ["src/main.rs"],
        "workspace": "D:/repo",
        "is_read_only": false,
        "is_destructive": false,
        "arguments": {
            "path": "src/main.rs",
            "content": {"preview": "fn main() {", "chars": 4096, "truncated": true},
            "token": "[REDACTED]"
        },
        "acknowledged": false,
        "seconds_remaining": 30.0
    })
}

#[test]
fn la_peticion_de_permiso_llega_entera_a_la_interfaz() {
    // Aprobar a ciegas no es aprobar: la vista tiene que llevar qué se hace,
    // sobre qué, con qué argumentos y con qué consecuencias.
    let mut vista = base();

    vista.aplicar(&permiso(carga_permiso()));

    let pendiente = vista.permiso("req-1").expect("la petición se registra");
    assert_eq!(pendiente.herramienta, "write_file");
    assert_eq!(pendiente.operacion, "write");
    assert_eq!(pendiente.accion, "escribir src/main.rs");
    assert_eq!(pendiente.riesgo, "medium");
    assert_eq!(pendiente.nivel, "R2");
    assert_eq!(pendiente.motivo, "aplicar el cambio pedido");
    assert_eq!(pendiente.recursos, vec!["src/main.rs".to_owned()]);
    assert_eq!(pendiente.workspace, "D:/repo");
    assert_eq!(pendiente.efectos.len(), 1);
    assert!(!pendiente.solo_lectura);
    assert!(!pendiente.caducado);
    assert_eq!(vista.fase, Some(FaseRun::WaitingPermission));
}

#[test]
fn los_argumentos_se_muestran_resumidos_y_redactados() {
    // El saneado lo hace Athena, que es quien tiene el valor original. Aquí se
    // comprueba que esta capa lo entiende y no reconstruye nada por su cuenta.
    let mut vista = base();

    vista.aplicar(&permiso(carga_permiso()));

    let pendiente = vista.permiso("req-1").expect("la petición se registra");
    let contenido = pendiente
        .argumentos
        .iter()
        .find(|a| a.nombre == "content")
        .expect("el contenido aparece");
    assert_eq!(contenido.valor, "fn main() {");
    assert_eq!(contenido.caracteres, Some(4096));
    assert!(contenido.resumido);

    let secreto = pendiente
        .argumentos
        .iter()
        .find(|a| a.nombre == "token")
        .expect("el argumento sensible aparece");
    assert!(secreto.redactado, "un valor redactado se señala como tal");

    let serializado = serde_json::to_string(&vista).expect("serializable");
    assert!(
        !serializado.contains("fn main() {\\n"),
        "no viaja el fichero entero, solo su principio"
    );
}

#[test]
fn una_peticion_sin_plazo_se_marca_caducada() {
    let mut vista = base();
    let mut carga = carga_permiso();
    carga["seconds_remaining"] = json!(0.0);

    vista.aplicar(&permiso(carga));

    let pendiente = vista.permiso("req-1").expect("la petición se registra");
    assert!(
        pendiente.caducado,
        "sin tiempo restante ya no se puede responder"
    );
}

#[test]
fn cancelar_el_run_retira_sus_preguntas() {
    // Responder a la pregunta de un run cancelado no haría nada; dejarla en
    // pantalla solo invita a intentarlo.
    let mut vista = base();
    vista.aplicar(&permiso(carga_permiso()));

    vista.aplicar(&evento("agent.cancelled", None, json!({})));

    assert!(vista.permisos.is_empty());
    assert_eq!(vista.fase, Some(FaseRun::Cancelled));
}

#[test]
fn terminar_o_fallar_tambien_retira_las_preguntas() {
    let mut completado = base();
    completado.aplicar(&permiso(carga_permiso()));
    completado.aplicar(&evento(
        "agent.completed",
        None,
        json!({"status": "completed"}),
    ));
    assert!(completado.permisos.is_empty());

    let mut fallado = base();
    fallado.aplicar(&permiso(carga_permiso()));
    fallado.aplicar(&evento("agent.failed", None, json!({"error": "boom"})));
    assert!(fallado.permisos.is_empty());
}

#[test]
fn responder_retira_la_peticion_una_sola_vez() {
    // Es lo que impide que un segundo clic mande una segunda respuesta.
    let mut vista = base();
    vista.aplicar(&permiso(carga_permiso()));

    assert!(vista.retirar_permiso("req-1"), "la primera vez estaba");
    assert!(!vista.retirar_permiso("req-1"), "la segunda ya no");
    assert!(vista.permiso("req-1").is_none());
}

#[test]
fn al_reconectar_las_peticiones_vivas_vuelven_completas() {
    // La reconexión no se apoya en los eventos perdidos: Athena manda su estado
    // y la proyección lo adopta tal cual, con los mismos datos que el evento.
    let mut vista = base();
    let marco = json!({
        "subscriber_id": "sub-1",
        "controls": true,
        "wire_version": 1,
        "snapshot": null,
        "pending_approvals": [{
            "request_id": "req-9",
            "run_id": "run-1",
            "tool_name": "run_command",
            "operation": "execute",
            "action": "ejecutar pytest",
            "risk": "high",
            "tier": "R3",
            "reason": "verificar los cambios",
            "possible_effects": ["ejecuta un proceso"],
            "resources": ["pytest"],
            "workspace": "D:/repo",
            "is_read_only": false,
            "is_destructive": false,
            "acknowledged": true,
            "seconds_remaining": 120.0,
            "arguments": {"command": "pytest -q"}
        }]
    });
    let marco: MarcoEstado = serde_json::from_value(marco).expect("marco válido");

    vista.aplicar(&MensajeFlujo::Estado(Box::new(marco)));

    let pendiente = vista.permiso("req-9").expect("vuelve tras reconectar");
    assert_eq!(pendiente.herramienta, "run_command");
    assert_eq!(pendiente.nivel, "R3");
    assert!(pendiente.confirmado);
    assert!(!pendiente.caducado);
    assert_eq!(pendiente.argumentos.len(), 1);
    assert_eq!(pendiente.argumentos[0].valor, "pytest -q");
}

#[test]
fn un_marco_con_campos_que_no_conocemos_se_sigue_leyendo() {
    // Athena puede añadir información al marco de estado —ahora manda `shape`, con qué
    // forma se decidió ejecutar el run y por qué— y una versión anterior de la aplicación
    // tiene que seguir funcionando. Si esto rompiese, actualizar el runtime dejaría la
    // aplicación sin flujo hasta actualizarla también.
    let marco = json!({
        "subscriber_id": "sub-1",
        "controls": true,
        "wire_version": 1,
        "shape": {
            "execution_mode": "auto",
            "executed_as": "direct",
            "reason": "auto -> direct: el plan no repartía trabajo"
        },
        "snapshot": null,
        "pending_approvals": []
    });

    let marco: MarcoEstado = serde_json::from_value(marco).expect("marco válido");

    assert_eq!(marco.subscriber_id, "sub-1");
    assert!(marco.controls);
}

#[test]
fn el_plan_de_una_instantanea_no_duplica_las_tareas_que_llegan_despues() {
    // Reconectar a mitad de un plan es lo normal: se cierra el portátil, se va la red,
    // se reinicia la aplicación. Lo que no puede pasar es que la misma tarea salga dos
    // veces —una del plan y otra del evento— y se lea como trabajo duplicado.
    let mut vista = base();
    let mut snapshot = snapshot_json("running");
    snapshot["working_memory"] = json!({
        "objective": "Arreglar calc.add",
        "current_plan": [
            {"description": "investigar el fallo", "status": "done", "task_id": "T01"},
            {"description": "corregir la función", "status": "pending", "task_id": "T02"}
        ],
        "current_step": 1
    });

    vista.aplicar(&marco_estado("sub-1", true, Some(snapshot)));
    assert_eq!(vista.tareas.len(), 2);
    assert_eq!(vista.tareas[0].id, "T01");

    vista.aplicar(&evento(
        "task.started",
        Some("T02"),
        json!({"task_id": "T02", "role": "coder", "dependencies": ["T01"]}),
    ));

    assert_eq!(vista.tareas.len(), 2, "la del plan y la del evento son la misma");
    let segunda = vista
        .tareas
        .iter()
        .find(|tarea| tarea.id == "T02")
        .expect("T02 sigue ahí");
    assert_eq!(segunda.estado, EstadoTarea::Running);
    // Lo que la instantánea no podía saber lo aporta el evento, sin perder el nombre
    // que el plan ya había dado.
    assert_eq!(segunda.rol, "coder");
    assert_eq!(segunda.dependencias, vec!["T01".to_owned()]);
    assert_eq!(segunda.nombre, "corregir la función");
}

#[test]
fn un_plan_sin_identidad_sigue_dibujandose() {
    // El plan que el propio bucle escribe para sí mismo no tiene identificadores: sus
    // pasos son prosa. Debe seguir viéndose, numerado, como hasta ahora.
    let mut vista = base();
    let mut snapshot = snapshot_json("running");
    snapshot["working_memory"] = json!({
        "objective": "Arreglar calc.add",
        "current_plan": [{"description": "leer calc.py", "status": "in_progress"}],
        "current_step": 0
    });

    vista.aplicar(&marco_estado("sub-1", true, Some(snapshot)));

    let paso = vista.tareas.first().expect("el paso se dibuja");
    assert_eq!(paso.id, "paso-0");
    assert_eq!(paso.estado, EstadoTarea::Running);
    assert!(paso.rol.is_empty());
}

#[test]
fn un_run_jerarquico_dibuja_su_plan_antes_de_ejecutarlo() {
    // El grafo se anuncia entero al empezar. Descubrirlo tarea a tarea dejaría a
    // quien mira sin saber cuánto queda.
    let mut vista = base();

    vista.aplicar(&evento("graph.started", None, json!({"tasks": 4})));

    assert_eq!(vista.fase, Some(FaseRun::Running));
    assert!(vista
        .actividad
        .iter()
        .any(|linea| linea.contains("4 tareas")));
}

#[test]
fn una_tarea_del_grafo_lleva_su_rol_y_sus_dependencias() {
    // Con las dependencias se puede dibujar un grafo; sin ellas, sólo una lista.
    // Y vienen de Athena: la interfaz no infiere ninguna.
    let mut vista = base();

    vista.aplicar(&evento(
        "task.started",
        Some("T02"),
        json!({
            "task_id": "T02",
            "role": "coder",
            "goal": "arreglar calc.add",
            "dependencies": ["T01"]
        }),
    ));

    let tarea = vista.tareas.first().expect("la tarea se registra");
    assert_eq!(tarea.id, "T02");
    assert_eq!(tarea.rol, "coder");
    assert_eq!(tarea.nombre, "arreglar calc.add");
    assert_eq!(tarea.dependencias, vec!["T01".to_owned()]);
    assert_eq!(tarea.estado, EstadoTarea::Running);
}

#[test]
fn reentregar_el_inicio_de_una_tarea_no_la_duplica() {
    // Una reconexión puede repetir el evento, y una tarea duplicada en la vista
    // se lee como trabajo duplicado.
    let mut vista = base();
    let inicio = evento(
        "task.started",
        Some("T01"),
        json!({"task_id": "T01", "goal": "mirar"}),
    );

    vista.aplicar(&inicio);
    vista.aplicar(&inicio);

    assert_eq!(vista.tareas.len(), 1);
}

#[test]
fn los_ficheros_de_una_tarea_son_ficheros_del_run() {
    // Si no subieran, la vista diría que no se tocó nada mientras el grafo
    // cambiaba medio repositorio.
    let mut vista = base();
    vista.aplicar(&evento(
        "task.started",
        Some("T01"),
        json!({"task_id": "T01"}),
    ));

    vista.aplicar(&evento(
        "task.completed",
        Some("T01"),
        json!({"task_id": "T01", "summary": "hecho", "files_changed": ["calc.py"]}),
    ));

    let tarea = vista.tareas.first().expect("la tarea sigue ahí");
    assert_eq!(tarea.estado, EstadoTarea::Completed);
    assert_eq!(tarea.detalle.as_deref(), Some("hecho"));
    assert_eq!(vista.ficheros_modificados, vec!["calc.py".to_owned()]);
}

#[test]
fn una_tarea_fallida_se_marca_sin_tumbar_el_plan() {
    let mut vista = base();
    vista.aplicar(&evento(
        "task.started",
        Some("T01"),
        json!({"task_id": "T01"}),
    ));

    vista.aplicar(&evento(
        "task.failed",
        Some("T01"),
        json!({"task_id": "T01", "summary": "no compila"}),
    ));

    assert_eq!(vista.tareas[0].estado, EstadoTarea::Failed);
    assert_ne!(vista.fase, Some(FaseRun::Failed), "una tarea no es el plan");
}

#[test]
fn el_plan_termina_por_su_propio_evento() {
    let mut vista = base();
    vista.aplicar(&evento("graph.started", None, json!({"tasks": 1})));

    vista.aplicar(&evento("graph.completed", None, json!({})));

    assert_eq!(vista.fase, Some(FaseRun::Completed));
    assert!(vista.permisos.is_empty());
}

#[test]
fn cancelar_el_plan_no_se_confunde_con_que_falle() {
    let mut vista = base();

    vista.aplicar(&evento("graph.cancelled", None, json!({})));

    assert_eq!(vista.fase, Some(FaseRun::Cancelled));
}

#[test]
fn la_estrategia_de_ejecucion_llega_a_la_vista() {
    // Athena decide la forma antes de que nadie pueda suscribirse, así que viaja en el
    // marco de estado. Aquí sólo se comprueba que llega entera: la interfaz no interpreta
    // la decisión, la enseña.
    let mut vista = base();
    let marco = json!({
        "subscriber_id": "sub-1",
        "controls": true,
        "wire_version": 1,
        "shape": {
            "execution_mode": "auto",
            "executed_as": "direct",
            "reason_code": "planning_unavailable",
            "reason": "auto -> direct: this deployment has planning switched off",
            "policy_verdict": "decompose",
            "policy_explanation": "Decomposition is worth its overhead here: …",
            "criteria_met": ["multiple independently verifiable outputs"],
            "assumed_signals": ["has_meaningful_dependencies"]
        },
        "snapshot": null,
        "pending_approvals": []
    });
    let marco: MarcoEstado = serde_json::from_value(marco).expect("marco válido");

    vista.aplicar(&MensajeFlujo::Estado(Box::new(marco)));

    let estrategia = vista.estrategia.expect("la estrategia llega");
    assert_eq!(estrategia.solicitada, "auto");
    assert_eq!(estrategia.seleccionada, "direct");
    assert_eq!(estrategia.codigo, "planning_unavailable");
    // Lo que opinó la política se conserva aunque diga lo contrario de lo que se hizo:
    // enseñar sólo uno de los dos contaría mal por qué este run fue como fue.
    assert_eq!(estrategia.veredicto_politica, "decompose");
    assert_eq!(estrategia.criterios.len(), 1);
    assert_eq!(estrategia.senales_supuestas.len(), 1);
}

#[test]
fn un_run_sin_estrategia_anunciada_no_se_inventa_una() {
    // Un Athena anterior no manda `shape`. La vista se queda sin el bloque, que es
    // distinto de enseñar uno con valores por defecto.
    let mut vista = base();

    vista.aplicar(&marco_estado("sub-1", true, None));

    assert!(vista.estrategia.is_none());
}
