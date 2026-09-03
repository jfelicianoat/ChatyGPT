//! Pruebas del cliente de Athena contra el servicio simulado.

use serde_json::json;

use super::simulated::{AthenaSimulado, GuionFlujo, RespuestaGuion};
use super::*;
use crate::error::AppError;

fn instantanea(estado: &str) -> serde_json::Value {
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
        "tool_references": [
            {"uri": "athena-result://k1", "store_key": "k1", "media_type": "text/plain",
             "size_chars": 42}
        ],
        "checkpoints": [{"name": "started", "occurred_at": "2026-08-19T00:00:00+00:00",
                         "payload": {}}],
    })
}

fn cliente(simulado: &AthenaSimulado) -> AthenaClient {
    let cliente = AthenaClient::for_base_url(&simulado.url_base()).expect("url válida");
    cliente
        .replace_token(Some("token-de-prueba"))
        .expect("token válido");
    cliente
}

fn en_runtime<F: std::future::Future>(futuro: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(futuro)
}

// -- contrato y errores ---------------------------------------------------

#[test]
fn la_salud_rechaza_una_version_de_contrato_distinta() {
    let simulado = AthenaSimulado::arrancar();
    simulado.responder(
        "/v1/health",
        RespuestaGuion::ok(json!({"status": "ok", "wire_version": 99, "runs": 0})),
    );

    let resultado = en_runtime(cliente(&simulado).salud());

    // Un contrato que no entendemos debe detenerse aquí y no más adelante, con
    // campos vacíos que parecerían datos legítimos.
    assert!(matches!(resultado, Err(AppError::AthenaContract(_))));
}

#[test]
fn la_salud_acepta_la_version_soportada() {
    let simulado = AthenaSimulado::arrancar();
    simulado.responder(
        "/v1/health",
        RespuestaGuion::ok(json!({"status": "ok", "wire_version": 1, "runs": 2})),
    );

    let salud = en_runtime(cliente(&simulado).salud()).expect("salud legible");

    assert_eq!(salud.status, "ok");
    assert_eq!(salud.runs, 2);
}

#[test]
fn un_token_invalido_da_un_error_tipado() {
    let simulado = AthenaSimulado::arrancar();
    simulado.responder(
        "/v1/runs",
        RespuestaGuion::error(401, "unauthorized", "Bad token"),
    );

    let resultado = en_runtime(cliente(&simulado).listar_runs(None));

    assert!(matches!(resultado, Err(AppError::AthenaUnauthorized)));
}

#[test]
fn responder_dos_veces_al_mismo_permiso_se_distingue_de_un_fallo() {
    // Athena responde 409 `already_resolved` cuando la petición ya se contestó.
    // Si eso llegara como conflicto genérico, la interfaz alarmaría por algo
    // que solo significa «llegaste tarde, no ha pasado nada».
    let simulado = AthenaSimulado::arrancar();
    simulado.responder(
        "/approvals/",
        RespuestaGuion::error(409, "already_resolved", "This request was answered"),
    );

    let resultado = en_runtime(cliente(&simulado).resolver_permiso(
        "run-1",
        "req-1",
        DecisionPermiso::Permitir,
        "sub-1",
    ));

    assert!(matches!(resultado, Err(AppError::AthenaAlreadyResolved)));
}

#[test]
fn una_peticion_que_ya_no_existe_no_se_reporta_como_recurso_perdido() {
    // Athena descarta la petición al caducar o al terminar el run, así que su
    // 404 no dice «no encuentro el run»: dice «esa pregunta ya no está en pie».
    let simulado = AthenaSimulado::arrancar();
    simulado.responder(
        "/approvals/",
        RespuestaGuion::error(404, "not_found", "No such approval request"),
    );

    let resultado = en_runtime(cliente(&simulado).resolver_permiso(
        "run-1",
        "req-1",
        DecisionPermiso::Denegar,
        "sub-1",
    ));

    assert!(matches!(resultado, Err(AppError::AthenaRequestGone)));
}

#[test]
fn un_observador_que_intenta_aprobar_sigue_recibiendo_su_propio_error() {
    // El 403 no debe quedar absorbido por los casos nuevos: quien no controla
    // el run necesita saber que el problema es de control, no de plazo.
    let simulado = AthenaSimulado::arrancar();
    simulado.responder(
        "/approvals/",
        RespuestaGuion::error(403, "not_controller", "Another client controls this run"),
    );

    let resultado = en_runtime(cliente(&simulado).resolver_permiso(
        "run-1",
        "req-1",
        DecisionPermiso::Permitir,
        "sub-2",
    ));

    assert!(matches!(resultado, Err(AppError::AthenaNotController)));
}

#[test]
fn un_artefacto_expirado_se_distingue_de_uno_vacio() {
    let simulado = AthenaSimulado::arrancar();
    simulado.responder(
        "/v1/results/",
        RespuestaGuion::error(410, "tool_result_unavailable", "expiró"),
    );

    let resultado = en_runtime(cliente(&simulado).descargar_artefacto("k1"));

    assert!(matches!(resultado, Err(AppError::AthenaArtifactExpired(_))));
}

#[test]
fn aprobar_sin_ser_el_cliente_que_controla_es_un_error_propio() {
    let simulado = AthenaSimulado::arrancar();
    simulado.responder(
        "/approvals/",
        RespuestaGuion::error(403, "not_controller", "otro cliente manda"),
    );

    let resultado = en_runtime(cliente(&simulado).resolver_permiso(
        "run-1",
        "req-1",
        DecisionPermiso::Permitir,
        "suscriptor-ajeno",
    ));

    assert!(matches!(resultado, Err(AppError::AthenaNotController)));
}

#[test]
fn un_servicio_que_no_responde_es_un_fallo_de_transporte() {
    // Puerto cerrado a propósito: es el caso de "Athena no está levantada".
    let cliente = AthenaClient::for_base_url("http://127.0.0.1:1").expect("url válida");

    let resultado = en_runtime(cliente.salud());

    assert!(matches!(resultado, Err(AppError::AthenaTransport(_))));
}

#[test]
fn una_url_sin_esquema_valido_se_rechaza_al_construir() {
    assert!(matches!(
        AthenaClient::for_base_url("ftp://127.0.0.1:8770"),
        Err(AppError::InvalidAthenaUrl(_))
    ));
    assert!(matches!(
        AthenaClient::for_base_url("no-es-una-url"),
        Err(AppError::InvalidAthenaUrl(_))
    ));
}

// -- operaciones ----------------------------------------------------------

#[test]
fn crear_un_run_manda_las_capacidades_que_preguntan_por_defecto() {
    let simulado = AthenaSimulado::arrancar();
    simulado.responder(
        "/v1/runs",
        RespuestaGuion::creado(json!({
            "run_id": "run-1", "workspace_id": "ws-1", "writes": "ask", "exec": "ask"
        })),
    );

    let creado = en_runtime(cliente(&simulado).crear_run(
        "Arreglar calc.add",
        "D:/repo",
        &OpcionesRun::default(),
    ))
    .expect("run creado");

    assert_eq!(creado.run_id, "run-1");
    let peticion = simulado.peticiones().pop().expect("una petición");
    assert!(peticion.cuerpo.contains("\"writes\":\"ask\""));
    assert!(peticion.cuerpo.contains("\"exec\":\"ask\""));
    assert_eq!(
        peticion.autorizacion.as_deref(),
        Some("Bearer token-de-prueba")
    );
}

#[test]
fn crear_un_run_sin_objetivo_no_llega_a_salir() {
    let simulado = AthenaSimulado::arrancar();

    let resultado =
        en_runtime(cliente(&simulado).crear_run("   ", "D:/repo", &OpcionesRun::default()));

    assert!(matches!(resultado, Err(AppError::Validation(_))));
    assert!(
        simulado.peticiones().is_empty(),
        "una validación local no debe gastar una petición"
    );
}

#[test]
fn leer_un_run_devuelve_la_instantanea_completa() {
    let simulado = AthenaSimulado::arrancar();
    simulado.responder(
        "/v1/runs/run-1",
        RespuestaGuion::ok(instantanea("completed")),
    );

    let vista = en_runtime(cliente(&simulado).leer_run("run-1")).expect("instantánea");

    assert_eq!(vista.status, EstadoRun::Completed);
    assert!(vista.status.es_terminal());
    assert_eq!(vista.ficheros_modificados(), vec!["calc.py".to_owned()]);
    assert_eq!(vista.estado_verificacion(), Some("passed"));
    assert_eq!(vista.tool_references[0].store_key, "k1");
    assert_eq!(vista.checkpoints[0].name, "started");
}

#[test]
fn cancelar_un_run_solo_necesita_que_el_servicio_lo_acepte() {
    let simulado = AthenaSimulado::arrancar();
    simulado.responder(
        "/cancel",
        RespuestaGuion::ok(json!({"run_id": "run-1", "cancelling": true})),
    );

    en_runtime(cliente(&simulado).cancelar_run("run-1")).expect("cancelación aceptada");

    assert!(simulado.peticiones()[0].ruta.ends_with("/cancel"));
}

#[test]
fn los_runs_por_recuperar_se_piden_aparte() {
    let simulado = AthenaSimulado::arrancar();
    simulado.responder(
        "/v1/runs",
        RespuestaGuion::ok(json!({"runs": [{
            "run_id": "abandonado-1",
            "workspace_id": "ws-1",
            "status": "recovery_pending",
            "resumable": true,
            "degraded": false,
            "objective": "Trabajo a medias",
            "files_modified": ["calc.py"],
            "updated_at": "2026-08-19T00:00:00+00:00"
        }]})),
    );

    let runs = en_runtime(cliente(&simulado).runs_por_recuperar()).expect("listado");

    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, EstadoRun::RecoveryPending);
    assert!(runs[0].resumable);
    // Un run interrumpido no puede presentarse como terminado.
    assert!(!runs[0].status.es_terminal());
    assert!(simulado.peticiones()[0]
        .ruta
        .contains("status=recovery_pending"));
}

#[test]
fn reanudar_manda_la_carpeta_de_trabajo() {
    let simulado = AthenaSimulado::arrancar();
    simulado.responder(
        "/resume",
        RespuestaGuion::ok(json!({"run_id": "run-1", "resumed": true})),
    );

    en_runtime(cliente(&simulado).reanudar_run("run-1", "D:/repo")).expect("reanudado");

    assert!(simulado.peticiones()[0].cuerpo.contains("D:/repo"));
}

#[test]
fn un_estado_desconocido_no_rompe_la_lectura() {
    // Que Athena añada un estado no debe dejar la interfaz sin poder leer nada.
    let simulado = AthenaSimulado::arrancar();
    simulado.responder(
        "/v1/runs/run-1",
        RespuestaGuion::ok(instantanea("un_estado_del_futuro")),
    );

    let vista = en_runtime(cliente(&simulado).leer_run("run-1")).expect("instantánea");

    assert_eq!(vista.status, EstadoRun::Desconocido);
    assert!(!vista.status.es_terminal());
    assert!(!vista.status.esta_vivo());
}

// -- permisos -------------------------------------------------------------

#[test]
fn resolver_un_permiso_prueba_quien_manda() {
    let simulado = AthenaSimulado::arrancar();
    simulado.responder(
        "/approvals/",
        RespuestaGuion::ok(json!({"request_id": "req-1", "decision": "allow"})),
    );

    en_runtime(cliente(&simulado).resolver_permiso(
        "run-1",
        "req-1",
        DecisionPermiso::Permitir,
        "sus-123",
    ))
    .expect("permiso resuelto");

    let peticion = &simulado.peticiones()[0];
    assert_eq!(peticion.suscriptor.as_deref(), Some("sus-123"));
    assert!(peticion.cuerpo.contains("\"decision\":\"allow\""));
}

#[test]
fn confirmar_recepcion_devuelve_el_tiempo_que_queda() {
    let simulado = AthenaSimulado::arrancar();
    simulado.responder(
        "/ack",
        RespuestaGuion::ok(json!({
            "request_id": "req-1", "run_id": "run-1", "tool_name": "edit_file",
            "action": "replace 1 occurrence(s) in calc.py", "risk": "medium",
            "tier": "r1_workspace_write", "reason": "quiere escribir",
            "possible_effects": ["Modifica calc.py"], "resources": ["calc.py"],
            "is_read_only": false, "is_destructive": false, "workspace": "D:/repo",
            "acknowledged": true, "seconds_remaining": 300.0
        })),
    );

    let pendiente =
        en_runtime(cliente(&simulado).confirmar_recepcion_permiso("run-1", "req-1", "sus-123"))
            .expect("acuse aceptado");

    assert!(pendiente.acknowledged);
    // El reloj humano solo arranca aquí, así que el tiempo restante es el largo.
    assert_eq!(pendiente.seconds_remaining, 300.0);
    assert_eq!(
        pendiente.possible_effects,
        vec!["Modifica calc.py".to_owned()]
    );
}

// -- flujo de eventos -----------------------------------------------------

#[test]
fn el_flujo_entrega_estado_y_luego_eventos() {
    let simulado = AthenaSimulado::arrancar();
    simulado.emitir(GuionFlujo {
        marcos: vec![
            GuionFlujo::marco_estado("sus-1", true, instantanea("running")),
            GuionFlujo::marco_evento("tool.started", "run-1", json!({"tool_name": "read_file"})),
            GuionFlujo::marco_evento("agent.completed", "run-1", json!({"iterations": 2})),
        ],
        cortar_al_final: true,
        retardo: None,
    });

    let mut recibidos: Vec<String> = Vec::new();
    let mut suscriptor = None;
    let cliente = cliente(&simulado);
    let mut flujo = cliente
        .flujo_eventos("run-1", true)
        .con_reconexion(OpcionesReconexion {
            intentos_maximos: Some(1),
            ..OpcionesReconexion::default()
        });

    en_runtime(flujo.escuchar(|mensaje| match mensaje {
        MensajeFlujo::Estado(estado) => {
            suscriptor = Some(estado.subscriber_id.clone());
            recibidos.push("estado".to_owned());
            true
        }
        MensajeFlujo::Evento(evento) => {
            let final_ = evento.es_final();
            recibidos.push(evento.name.clone());
            // Parar en el evento final es decisión de quien escucha.
            !final_
        }
    }))
    .expect("flujo leído");

    assert_eq!(recibidos, vec!["estado", "tool.started", "agent.completed"]);
    assert_eq!(suscriptor.as_deref(), Some("sus-1"));
    assert_eq!(flujo.suscriptor(), Some("sus-1"));
}

#[test]
fn al_reconectar_se_pide_reanudar_por_el_ultimo_evento_visto() {
    // Sin esto, dos segundos sin red cuestan una resincronización entera y la
    // vista tira todo lo que había derivado. El servidor sabe reanudar desde
    // ADR-021; esto es lo que hace que alguien se lo pida.
    let simulado = AthenaSimulado::arrancar();
    simulado.emitir(GuionFlujo {
        marcos: vec![
            GuionFlujo::marco_estado("sus-1", true, instantanea("running")),
            GuionFlujo::marco_evento("tool.started", "run-1", json!({"tool_name": "grep"})),
        ],
        cortar_al_final: true,
        retardo: None,
    });
    simulado.emitir(GuionFlujo {
        marcos: vec![
            GuionFlujo::marco_estado("sus-2", true, instantanea("completed")),
            GuionFlujo::marco_evento("agent.completed", "run-1", json!({})),
        ],
        cortar_al_final: true,
        retardo: None,
    });

    let cliente = cliente(&simulado);
    let mut flujo = cliente
        .flujo_eventos("run-1", true)
        .con_reconexion(OpcionesReconexion {
            espera_inicial: std::time::Duration::from_millis(20),
            espera_maxima: std::time::Duration::from_millis(50),
            intentos_maximos: Some(3),
        });
    let mut vistos = 0;
    let _ = en_runtime(flujo.escuchar(|mensaje| {
        if let MensajeFlujo::Evento(evento) = &mensaje {
            vistos += 1;
            return evento.name != "agent.completed";
        }
        true
    }));

    let peticiones = simulado.peticiones();
    let reconexion = peticiones
        .iter()
        .filter(|peticion| peticion.ruta.contains("/events"))
        .nth(1)
        .expect("hubo una segunda conexión");
    assert_eq!(
        reconexion.metodo, "GET",
        "el flujo se abre leyendo, no enviando"
    );
    assert!(
        reconexion.cabecera("last-event-id").is_some(),
        "la reconexión dice por dónde iba"
    );
    assert_eq!(vistos, 2);
}

#[test]
fn la_primera_conexion_no_pide_reanudar_nada() {
    // Pedir reanudación desde un punto inventado haría que Athena respondiera
    // con una instantánea igualmente, pero mintiendo sobre lo que se sabe.
    let simulado = AthenaSimulado::arrancar();
    simulado.emitir(GuionFlujo {
        marcos: vec![GuionFlujo::marco_estado(
            "sus-1",
            true,
            instantanea("completed"),
        )],
        cortar_al_final: true,
        retardo: None,
    });

    let cliente = cliente(&simulado);
    let mut flujo = cliente
        .flujo_eventos("run-1", true)
        .con_reconexion(OpcionesReconexion {
            espera_inicial: std::time::Duration::from_millis(10),
            espera_maxima: std::time::Duration::from_millis(20),
            intentos_maximos: Some(1),
        });
    let _ = en_runtime(flujo.escuchar(|_| false));

    let peticiones = simulado.peticiones();
    let primera = peticiones
        .iter()
        .find(|peticion| peticion.ruta.contains("/events"))
        .expect("se conectó");
    assert!(primera.cabecera("last-event-id").is_none());
}

#[test]
fn el_flujo_reconecta_y_recibe_una_instantanea_nueva() {
    // Athena manda estado y luego cola: los eventos perdidos en el corte no son
    // un problema de corrección porque la instantánea siguiente los sustituye.
    let simulado = AthenaSimulado::arrancar();
    simulado.emitir(GuionFlujo {
        marcos: vec![GuionFlujo::marco_estado(
            "sus-1",
            true,
            instantanea("running"),
        )],
        cortar_al_final: true,
        retardo: None,
    });
    simulado.emitir(GuionFlujo {
        marcos: vec![
            GuionFlujo::marco_estado("sus-2", true, instantanea("completed")),
            GuionFlujo::marco_evento("agent.completed", "run-1", json!({})),
        ],
        cortar_al_final: true,
        retardo: None,
    });

    let mut estados: Vec<String> = Vec::new();
    let cliente = cliente(&simulado);
    let mut flujo = cliente
        .flujo_eventos("run-1", true)
        .con_reconexion(OpcionesReconexion {
            espera_inicial: std::time::Duration::from_millis(20),
            espera_maxima: std::time::Duration::from_millis(50),
            intentos_maximos: Some(3),
        });

    en_runtime(flujo.escuchar(|mensaje| match mensaje {
        MensajeFlujo::Estado(estado) => {
            estados.push(estado.subscriber_id.clone());
            true
        }
        MensajeFlujo::Evento(evento) => !evento.es_final(),
    }))
    .expect("flujo leído");

    assert_eq!(estados, vec!["sus-1", "sus-2"]);
    assert_eq!(flujo.suscriptor(), Some("sus-2"));
}

#[test]
fn un_flujo_no_autorizado_no_se_reintenta() {
    // Reintentar con un token inválido solo repite el rechazo.
    let simulado = AthenaSimulado::arrancar();
    let cliente = cliente(&simulado);
    let mut flujo = cliente
        .flujo_eventos("run-1", true)
        .con_reconexion(OpcionesReconexion {
            espera_inicial: std::time::Duration::from_millis(10),
            espera_maxima: std::time::Duration::from_millis(10),
            intentos_maximos: Some(5),
        });

    // Sin guion de flujo, el simulado responde 404 al no encontrar ruta.
    let resultado = en_runtime(flujo.escuchar(|_| true));

    assert!(matches!(resultado, Err(AppError::NotFound(_))));
    assert_eq!(
        simulado.peticiones().len(),
        1,
        "un run inexistente no se reintenta"
    );
}

#[test]
fn un_marco_ilegible_no_tumba_el_flujo() {
    let simulado = AthenaSimulado::arrancar();
    simulado.emitir(GuionFlujo {
        marcos: vec![
            "event: event\ndata: {esto no es json}\n\n".to_owned(),
            GuionFlujo::marco_evento("agent.completed", "run-1", json!({})),
        ],
        cortar_al_final: true,
        retardo: None,
    });

    let mut vistos: Vec<String> = Vec::new();
    let cliente = cliente(&simulado);
    let mut flujo = cliente
        .flujo_eventos("run-1", false)
        .con_reconexion(OpcionesReconexion {
            intentos_maximos: Some(1),
            ..OpcionesReconexion::default()
        });

    en_runtime(flujo.escuchar(|mensaje| {
        if let MensajeFlujo::Evento(evento) = mensaje {
            vistos.push(evento.name.clone());
            return !evento.es_final();
        }
        true
    }))
    .expect("flujo leído");

    assert_eq!(vistos, vec!["agent.completed"]);
}

#[test]
fn un_evento_de_permiso_se_reconoce_y_trae_su_identificador() {
    let simulado = AthenaSimulado::arrancar();
    simulado.emitir(GuionFlujo {
        marcos: vec![
            GuionFlujo::marco_estado("sus-1", true, instantanea("running")),
            GuionFlujo::marco_evento(
                "permission.requested",
                "run-1",
                json!({"request_id": "req-9", "awaiting_decision": true,
                       "action": "replace 1 occurrence(s) in calc.py"}),
            ),
        ],
        cortar_al_final: true,
        retardo: None,
    });

    let mut peticion = None;
    let cliente = cliente(&simulado);
    let mut flujo = cliente
        .flujo_eventos("run-1", true)
        .con_reconexion(OpcionesReconexion {
            intentos_maximos: Some(1),
            ..OpcionesReconexion::default()
        });

    en_runtime(flujo.escuchar(|mensaje| {
        if let MensajeFlujo::Evento(evento) = mensaje {
            if evento.pide_permiso() {
                peticion = evento.identificador_peticion().map(str::to_owned);
                return false;
            }
        }
        true
    }))
    .expect("flujo leído");

    assert_eq!(peticion.as_deref(), Some("req-9"));
}

// -- credenciales ---------------------------------------------------------

#[test]
fn el_token_se_puede_rotar_sin_reconstruir_el_cliente() {
    // Athena regenera su token en cada arranque, así que esto no es un lujo.
    let simulado = AthenaSimulado::arrancar();
    let cliente = AthenaClient::for_base_url(&simulado.url_base()).expect("url válida");

    assert!(cliente.token_actual().is_none());
    cliente
        .replace_token(Some("primero"))
        .expect("token válido");
    assert!(cliente.token_actual().is_some());
    cliente
        .replace_token(Some("segundo"))
        .expect("token válido");
    simulado.responder(
        "/v1/health",
        RespuestaGuion::ok(json!({"status": "ok",
        "wire_version": 1, "runs": 0})),
    );

    en_runtime(cliente.salud()).expect("salud legible");

    assert_eq!(
        simulado.peticiones()[0].autorizacion.as_deref(),
        Some("Bearer segundo")
    );
}

#[test]
fn quitar_el_token_deja_de_mandar_la_cabecera() {
    let simulado = AthenaSimulado::arrancar();
    let cliente = cliente(&simulado);
    cliente.replace_token(None).expect("sin token");
    simulado.responder(
        "/v1/health",
        RespuestaGuion::ok(json!({"status": "ok", "wire_version": 1, "runs": 0})),
    );

    en_runtime(cliente.salud()).expect("salud legible");

    assert!(simulado.peticiones()[0].autorizacion.is_none());
}

// -- revision del encargo -------------------------------------------------

#[test]
fn revisar_el_encargo_dice_siempre_sobre_que_revision_escribe() {
    let simulado = AthenaSimulado::arrancar();
    simulado.responder(
        "/goal",
        RespuestaGuion::ok(json!({
            "run_id": "run-1",
            "goal": {"text": "Nuevo encargo", "revision": 2, "reason": "faltaba algo",
                     "revised_at": "2026-08-22T10:00:00+00:00"},
            "applied": false,
        })),
    );

    let resultado = en_runtime(cliente(&simulado).revisar_objetivo(
        "run-1",
        "Nuevo encargo",
        1,
        "faltaba algo",
    ))
    .expect("revision aceptada");

    match resultado {
        RevisionObjetivo::Aceptada { objetivo } => assert_eq!(objetivo.revision, 2),
        otro => panic!("se esperaba aceptada: {otro:?}"),
    }
    // `base_revision` no tiene defecto ni aqui ni en Athena: uno implicito
    // convertiria cada revision en un pisoton.
    let cuerpo = &simulado.peticiones()[0].cuerpo;
    assert!(cuerpo.contains("\"base_revision\":1"), "{cuerpo}");
}

#[test]
fn un_conflicto_de_revision_es_una_respuesta_y_no_un_error() {
    // Que otro haya escrito antes es algo que pasa, no un fallo de quien lo intenta.
    // Devolverlo como `AppError` obligaria a la interfaz a leer una frase para
    // enterarse, y a reintentar a ciegas para recuperarse.
    let simulado = AthenaSimulado::arrancar();
    simulado.responder(
        "/goal",
        RespuestaGuion {
            estado: 409,
            cuerpo: json!({
                "error": {"code": "goal_conflict", "message": "va por la revision 3"},
                "current_revision": 3,
                "current": "Lo que pidio Telegram",
            })
            .to_string()
            .into_bytes(),
            tipo: "application/json",
        },
    );
    // La relectura que sigue al conflicto: el 409 trae texto y revision, pero no el
    // motivo ni la fecha, y ensenar media verdad es peor que preguntar otra vez.
    simulado.responder(
        "/goal",
        RespuestaGuion::ok(json!({
            "text": "Lo que pidio Telegram", "revision": 3, "reason": "cambio de alcance",
            "revised_at": "2026-08-22T10:05:00+00:00",
        })),
    );

    let resultado =
        en_runtime(cliente(&simulado).revisar_objetivo("run-1", "Lo que quiero yo", 2, ""))
            .expect("el conflicto no es un error de transporte");

    match resultado {
        RevisionObjetivo::Conflicto { vigente } => {
            assert_eq!(vigente.revision, 3);
            assert_eq!(vigente.text, "Lo que pidio Telegram");
            assert_eq!(vigente.reason, "cambio de alcance");
        }
        otro => panic!("se esperaba conflicto: {otro:?}"),
    }
}

#[test]
fn si_la_relectura_falla_se_contesta_con_lo_que_traia_el_conflicto() {
    // Media verdad es preferible a ninguna cuando lo que falta es el adorno: sin
    // revision y sin texto la interfaz no podria ni decir que paso.
    let simulado = AthenaSimulado::arrancar();
    simulado.responder(
        "/goal",
        RespuestaGuion {
            estado: 409,
            cuerpo: json!({
                "error": {"code": "goal_conflict", "message": "va por la revision 3"},
                "current_revision": 3,
                "current": "Lo que pidio Telegram",
            })
            .to_string()
            .into_bytes(),
            tipo: "application/json",
        },
    );

    let resultado =
        en_runtime(cliente(&simulado).revisar_objetivo("run-1", "Lo que quiero yo", 2, ""))
            .expect("el conflicto se conserva aunque la relectura falle");

    match resultado {
        RevisionObjetivo::Conflicto { vigente } => {
            assert_eq!(vigente.revision, 3);
            assert!(vigente.reason.is_empty());
        }
        otro => panic!("se esperaba conflicto: {otro:?}"),
    }
}

#[test]
fn un_409_que_no_es_de_revision_no_se_disfraza_de_conflicto_de_encargo() {
    // Un run que ya termino tambien contesta 409, y refrescar el objetivo no lo
    // arreglaria: ofrecer «vuelve a escribirlo» seria mandar a repetir algo imposible.
    let simulado = AthenaSimulado::arrancar();
    simulado.responder(
        "/goal",
        RespuestaGuion::error(409, "run_finished", "That run is over"),
    );

    let resultado = en_runtime(cliente(&simulado).revisar_objetivo("run-1", "Otra cosa", 1, ""));

    assert!(matches!(resultado, Err(AppError::Conflict(_))));
}

#[test]
fn un_encargo_vacio_no_llega_a_salir_de_la_aplicacion() {
    let simulado = AthenaSimulado::arrancar();

    let resultado = en_runtime(cliente(&simulado).revisar_objetivo("run-1", "   ", 1, ""));

    assert!(matches!(resultado, Err(AppError::Validation(_))));
    assert!(simulado.peticiones().is_empty());
}

// -- modelos --------------------------------------------------------------

#[test]
fn los_modelos_los_dice_athena_y_no_una_lista_local() {
    let simulado = AthenaSimulado::arrancar();
    simulado.responder(
        "/v1/models",
        RespuestaGuion::ok(json!({
            "default": "qwen3.8:27b",
            "models": [
                {"name": "qwen3.8:27b", "default": true},
                {"name": "DeepSeek-V4-Pro", "default": false}
            ],
        })),
    );

    let listado = en_runtime(cliente(&simulado).listar_modelos()).expect("listado legible");

    assert_eq!(listado.default, "qwen3.8:27b");
    assert_eq!(listado.models.len(), 2);
    assert!(listado.models[0].default);
    assert_eq!(listado.models[1].name, "DeepSeek-V4-Pro");
}

#[test]
fn un_despliegue_sin_eleccion_de_modelo_no_es_un_error() {
    // Athena contesta 404 cuando corre con un modelo fijo. Eso no es un fallo del que
    // avisar a nadie: es una respuesta, y se traduce a «no hay nada que elegir».
    let simulado = AthenaSimulado::arrancar();
    simulado.responder(
        "/v1/models",
        RespuestaGuion::error(
            404,
            "models_fixed",
            "This deployment does not offer a choice",
        ),
    );

    let listado = en_runtime(cliente(&simulado).listar_modelos()).expect("404 no es un error");

    assert!(listado.models.is_empty());
    assert!(listado.default.is_empty());
}

#[test]
fn el_modelo_elegido_viaja_en_la_peticion_y_el_vacio_se_omite() {
    // Mandar `model: ""` seria pedir un modelo sin nombre, y Athena lo rechazaria con
    // razon. La ausencia del campo es lo que significa «decide tu».
    let simulado = AthenaSimulado::arrancar();
    // Dos guiones: cada respuesta se consume una vez y aqui se crean dos runs.
    for _ in 0..2 {
        simulado.responder(
            "/v1/runs",
            RespuestaGuion::creado(json!({
                "run_id": "run-1", "workspace_id": "ws-1", "writes": "ask", "exec": "ask"
            })),
        );
    }

    let elegido = OpcionesRun {
        modelo: "DeepSeek-V4-Pro".to_owned(),
        ..OpcionesRun::default()
    };
    en_runtime(cliente(&simulado).crear_run("arregla el bug", "C:/repo", &elegido))
        .expect("run creado");
    en_runtime(cliente(&simulado).crear_run("arregla el bug", "C:/repo", &OpcionesRun::default()))
        .expect("run creado");

    let peticiones = simulado.peticiones();
    assert!(peticiones[0]
        .cuerpo
        .contains("\"model\":\"DeepSeek-V4-Pro\""));
    assert!(
        !peticiones[1].cuerpo.contains("\"model\""),
        "sin eleccion el campo no viaja: una cadena vacia seria pedir un modelo sin nombre"
    );
}

// -- perfiles -------------------------------------------------------------

#[test]
fn los_perfiles_los_dice_athena_y_no_una_lista_local() {
    let simulado = AthenaSimulado::arrancar();
    simulado.responder(
        "/v1/profiles",
        RespuestaGuion::ok(json!({
            "default": "software_engineering",
            "profiles": [
                {"name": "software_engineering", "subject": "a repository",
                 "evidence": "executed_checks", "proves": "checks passed",
                 "tools": ["glob", "bash"], "description": "Repositorio"},
                {"name": "documents", "subject": "a folder of documents",
                 "evidence": "artifacts", "proves": "the deliverables exist",
                 "tools": ["glob", "write_file"], "description": "Documentos"}
            ],
        })),
    );

    let listado = en_runtime(cliente(&simulado).listar_perfiles()).expect("listado legible");

    assert_eq!(listado.default, "software_engineering");
    assert_eq!(listado.profiles.len(), 2);
    assert_eq!(listado.profiles[1].name, "documents");
    assert!(listado.profiles[1].proves.contains("deliverables"));
}

#[test]
fn un_perfil_desconocido_vuelve_como_rechazo_y_no_cae_al_de_por_defecto() {
    // Quien pide `documents` y recibe el de software no se entera hasta que Athena
    // intenta ejecutar los tests de una carpeta de textos. Athena lo rechaza con 400 y
    // el cliente lo deja pasar tal cual, sin buscarle un sustituto.
    let simulado = AthenaSimulado::arrancar();
    simulado.responder(
        "/v1/runs",
        RespuestaGuion::error(
            400,
            "tool_validation_error",
            "Perfil desconocido: inventado. Disponibles: documents, software_engineering",
        ),
    );

    let opciones = OpcionesRun {
        perfil: "inventado".to_owned(),
        ..OpcionesRun::default()
    };
    let resultado = en_runtime(cliente(&simulado).crear_run("Arreglar", "D:/repo", &opciones));

    match resultado {
        Err(AppError::Validation(mensaje)) => assert!(mensaje.contains("Disponibles")),
        otro => panic!("se esperaba el rechazo de Athena: {otro:?}"),
    }
}

#[test]
fn sin_perfil_elegido_no_se_manda_ninguno() {
    // Mandar una cadena vacia seria pedir un perfil sin nombre, y Athena lo rechazaria
    // con razon. La ausencia del campo es lo que significa «el de por defecto».
    let simulado = AthenaSimulado::arrancar();
    simulado.responder(
        "/v1/runs",
        RespuestaGuion::creado(json!({
            "run_id": "run-1", "workspace_id": "ws-1", "writes": "ask", "exec": "ask",
        })),
    );

    en_runtime(cliente(&simulado).crear_run("Arreglar", "D:/repo", &OpcionesRun::default()))
        .expect("run creado");

    let cuerpo = &simulado.peticiones()[0].cuerpo;
    assert!(!cuerpo.contains("profile"), "{cuerpo}");
}

// -- memoria de proyecto --------------------------------------------------

#[test]
fn la_memoria_se_pide_por_proyecto_y_llega_con_su_procedencia() {
    let simulado = AthenaSimulado::arrancar();
    simulado.responder(
        "/v1/memory",
        RespuestaGuion::ok(json!({
            "project_id": "ws-1",
            "items": [{
                "id": "mem-1", "project_id": "ws-1", "kind": "verified_command",
                "content": "pytest -q", "source": "run:ws-1", "source_reference": null,
                "confidence": 0.9, "verification_state": "verified", "scope": "project",
                "status": "active", "supersedes": null,
                "created_at": "2026-08-20T10:00:00+00:00",
                "updated_at": "2026-08-20T10:00:00+00:00", "stale": false,
            }],
        })),
    );

    let recuerdos = en_runtime(cliente(&simulado).listar_memoria("ws-1", 50)).expect("legible");

    assert_eq!(recuerdos.len(), 1);
    assert_eq!(recuerdos[0].verification_state, "verified");
    assert_eq!(recuerdos[0].status, "active");
    assert!(!recuerdos[0].stale);
    assert!(simulado.peticiones()[0].ruta.contains("project=ws-1"));
}

#[test]
fn sin_proyecto_no_se_pregunta_por_la_memoria_de_nadie() {
    // Athena exige el proyecto y responderia 400. Preguntarlo igualmente gastaria una
    // vuelta para enterarse de algo que ya se sabia aqui.
    let simulado = AthenaSimulado::arrancar();

    let resultado = en_runtime(cliente(&simulado).listar_memoria("  ", 50));

    assert!(matches!(resultado, Err(AppError::Validation(_))));
    assert!(simulado.peticiones().is_empty());
}

#[test]
fn confirmar_un_recuerdo_devuelve_el_estado_que_alcanzo() {
    // `user_confirmed` solo se alcanza por HTTP: Athena tiene una prueba que prohibe
    // que ningun modulo suyo lo nombre. Este es el camino entero.
    let simulado = AthenaSimulado::arrancar();
    simulado.responder(
        "/confirm",
        RespuestaGuion::ok(json!({
            "id": "mem-1", "project_id": "ws-1", "kind": "verified_command",
            "content": "pytest -q", "source": "run:ws-1", "confidence": 0.9,
            "verification_state": "user_confirmed", "scope": "project", "status": "active",
            "created_at": "2026-08-20T10:00:00+00:00",
            "updated_at": "2026-08-22T10:00:00+00:00",
        })),
    );

    let recuerdo = en_runtime(cliente(&simulado).confirmar_recuerdo("mem-1")).expect("legible");

    assert_eq!(recuerdo.verification_state, "user_confirmed");
    assert_eq!(simulado.peticiones()[0].metodo, "POST");
}

#[test]
fn olvidar_un_recuerdo_que_ya_no_esta_se_distingue_de_un_fallo() {
    let simulado = AthenaSimulado::arrancar();
    simulado.responder(
        "/v1/memory",
        RespuestaGuion::error(404, "not_found", "No memory mem-9"),
    );

    let resultado = en_runtime(cliente(&simulado).olvidar_recuerdo("mem-9"));

    assert!(matches!(resultado, Err(AppError::NotFound(_))));
}
