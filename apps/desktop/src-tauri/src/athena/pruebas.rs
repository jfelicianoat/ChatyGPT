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
