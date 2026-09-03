//! Ordenes del area de Athena: runs, memoria, objetivos y permisos.

use crate::*;

/// Estado del servicio de Athena para el área de la interfaz.
///
/// No propaga el error de red: que Athena no esté levantada no es un fallo de
/// ChatyGPT, es un estado que la interfaz debe poder enseñar. El chat normal
/// sigue funcionando contra el Broker.
#[tauri::command]
pub(crate) async fn get_athena_status(
    state: State<'_, AppState>,
) -> Result<athena::EstadoAreaAthena, AppError> {
    let configurada = secrets::athena_token_path(&state.data_dir).is_file();
    Ok(state.athena.estado(configurada).await)
}

/// Guarda el token de Athena cifrado para esta cuenta de Windows.
///
/// El valor nunca vuelve al frontend: solo se informa del estado resultante.
/// Athena regenera su token en cada arranque, así que esta orden se usará a
/// menudo y por eso rota la credencial en caliente en lugar de exigir reinicio.
#[tauri::command]
pub(crate) fn set_athena_credential(
    token: String,
    state: State<'_, AppState>,
) -> Result<secrets::BrokerCredentialStatus, AppError> {
    secrets::store_athena_token(&state.data_dir, &token)?;
    state.athena.cliente().replace_token(Some(token.trim()))?;
    logging::info("athena.credential_stored", None, &[]);
    Ok(secrets::athena_credential_status(&state.data_dir))
}

/// Retira la credencial guardada. Requiere confirmación explícita.
#[tauri::command]
pub(crate) fn clear_athena_credential(
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<secrets::BrokerCredentialStatus, AppError> {
    if !confirmed {
        return Err(AppError::Validation(
            "retirar la credencial de Athena requiere confirmación".to_owned(),
        ));
    }
    secrets::clear_athena_token(&state.data_dir)?;
    state.athena.cliente().replace_token(None)?;
    logging::info("athena.credential_cleared", None, &[]);
    Ok(secrets::athena_credential_status(&state.data_dir))
}

/// Abre un run sobre una carpeta que el usuario ya autorizó.
///
/// La autorización se comprueba aquí y Athena la vuelve a comprobar por su
/// cuenta con su propio límite de espacio de trabajo: dos controles
/// independientes, que es lo que hace que un fallo en uno no baste.
#[tauri::command]
pub(crate) async fn start_athena_run(
    objective: String,
    folder_id: String,
    writes: Option<String>,
    execution: Option<String>,
    profile: Option<String>,
    model: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    let objective = validated_text(&objective, "el objetivo", 2_000)?;
    let carpeta = state
        .database
        .list_authorized_folders()?
        .into_iter()
        .find(|folder| {
            folder.id == folder_id
                && folder.revoked_at.is_none()
                && folder.permissions.get("athena").and_then(|value| value.as_bool())
                    == Some(true)
        })
        .ok_or_else(|| {
            AppError::Validation(
                "la carpeta no está autorizada para Athena; añádela desde su sección antes de lanzar el trabajo".to_owned(),
            )
        })?;
    let opciones = athena::OpcionesRun {
        escrituras: modo_capacidad(writes.as_deref())?,
        ejecucion: modo_capacidad(execution.as_deref())?,
        // El nombre no se valida aquí: los perfiles los conoce Athena, y una lista
        // copiada en el cliente caducaría en silencio en cuanto el despliegue añadiera
        // uno. Un nombre que no exista vuelve como 400 de Athena, con los que sí existen.
        perfil: profile.unwrap_or_default().trim().to_owned(),
        // Igual que el perfil, y por lo mismo: los modelos los conoce Athena. Copiar
        // aquí una lista la dejaría caducada en cuanto el despliegue cambiara la suya.
        modelo: model.unwrap_or_default().trim().to_owned(),
        ..athena::OpcionesRun::default()
    };
    let run_id = state
        .athena
        .abrir(&objective, &carpeta.path, &opciones)
        .await?;
    // Lo único que ChatyGPT guarda del run es cómo volver a preguntarle a
    // Athena por él. El estado sigue siendo suyo.
    state
        .database
        .record_athena_run_started(&run_id, &objective, &carpeta.path)?;
    Ok(run_id)
}

/// Runs abiertos en una sesión anterior, para volver a engancharlos.
///
/// La lista es local; qué siguen siendo esos runs lo dice Athena. Un run que ya
/// terminó se cierra aquí en vez de quedarse colgado para siempre.
#[tauri::command]
pub(crate) async fn list_athena_tracked_runs(
    state: State<'_, AppState>,
) -> Result<Vec<athena::ProyeccionRun>, AppError> {
    let recordados = state.database.list_open_athena_runs()?;
    let mut vistas = Vec::with_capacity(recordados.len());
    for recordado in recordados {
        match state.athena.cliente().leer_run(&recordado.run_id).await {
            Ok(_) => {
                state.athena.readoptar(
                    &recordado.run_id,
                    &recordado.objetivo,
                    &recordado.workspace,
                );
                let vista = state.athena.refrescar(&recordado.run_id).await?;
                let fase = vista
                    .fase
                    .map(athena::FaseRun::palabra)
                    .unwrap_or("unknown");
                if vista.fase.is_some_and(athena::FaseRun::es_terminal) {
                    state.database.close_athena_run(&recordado.run_id, fase)?;
                } else {
                    state
                        .database
                        .record_athena_run_phase(&recordado.run_id, fase)?;
                }
                vistas.push(vista);
            }
            // Athena ya no lo conoce: se olvida en lugar de reaparecer en cada arranque.
            Err(AppError::NotFound(_)) => {
                state
                    .database
                    .close_athena_run(&recordado.run_id, "unknown")?;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(vistas)
}

/// Traduce el modo pedido por la interfaz. Ausente significa preguntar.
pub(crate) fn modo_capacidad(valor: Option<&str>) -> Result<athena::ModoCapacidad, AppError> {
    Ok(match valor {
        None | Some("ask") => athena::ModoCapacidad::Ask,
        Some("off") => athena::ModoCapacidad::Off,
        Some("allow") => athena::ModoCapacidad::Allow,
        Some(otro) => {
            return Err(AppError::Validation(format!(
                "modo de capacidad desconocido: {otro}"
            )))
        }
    })
}

/// Proyección de un run. Es lo que el área muestra, y procede solo de los
/// eventos y las instantáneas que publicó Athena.
#[tauri::command]
pub(crate) async fn get_athena_run(
    run_id: String,
    state: State<'_, AppState>,
) -> Result<athena::ProyeccionRun, AppError> {
    // El flujo da inmediatez, pero la instantánea durable decide. Una conexión
    // SSE aún abierta no demuestra que el evento terminal haya llegado: si se
    // pierde justo al cerrar, confiar en la caché dejaría la UI en «Trabajando».
    let vista = state.athena.consultar(&run_id).await?;
    cerrar_si_termino(&state, &run_id, &vista)?;
    Ok(vista)
}

/// Cierra el apunte local en cuanto el run acaba, para que la lista de runs por
/// re-enganchar no arrastre trabajo ya terminado.
pub(crate) fn cerrar_si_termino(
    state: &State<'_, AppState>,
    run_id: &str,
    vista: &athena::ProyeccionRun,
) -> Result<(), AppError> {
    if let Some(fase) = vista.fase.filter(|fase| fase.es_terminal()) {
        state.database.close_athena_run(run_id, fase.palabra())?;
    }
    Ok(())
}

/// Todos los runs que Athena recuerda, para poder abrir uno de antes.
///
/// La lista es de Athena y no del apunte local: lo que ChatyGPT guarda es cómo volver a
/// preguntar, no qué pasó. Un run lanzado desde Telegram aparece aquí igual, porque el
/// run es del runtime y no de quien lo pidió.
#[tauri::command]
pub(crate) async fn list_athena_runs(
    state: State<'_, AppState>,
) -> Result<Vec<athena::ResumenRun>, AppError> {
    state.athena.cliente().listar_runs(None).await
}

/// Lo que ocurrió en un run, reconstruido desde el registro duradero de Athena.
///
/// La proyección que devuelve se construye con el **mismo** lector que la vista en vivo:
/// un run releído se lee igual que se leyó cuando pasaba. Un segundo lector garantizaría
/// que antes o después los dos contaran cosas distintas del mismo run.
#[tauri::command]
pub(crate) async fn get_athena_run_history(
    run_id: String,
    state: State<'_, AppState>,
) -> Result<athena::HistoriaVista, AppError> {
    state.athena.historia(&run_id).await
}

/// Cuántos recuerdos se piden de una vez.
///
/// Athena tampoco devuelve la memoria entera: un panel que la cargara toda gastaría la
/// pantalla en cosas que nadie vino a mirar.
const LIMITE_MEMORIA_ATHENA: u32 = 50;

/// Lo que Athena cree saber de un proyecto.
///
/// Se pide por el identificador de espacio de trabajo, que es lo que el runtime usa como
/// proyecto. Pedirlo por ruta funcionaría hasta que dos máquinas montasen la misma
/// carpeta en sitios distintos.
#[tauri::command]
pub(crate) async fn list_athena_memory(
    workspace_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<athena::RecuerdoProyecto>, AppError> {
    state
        .athena
        .cliente()
        .listar_memoria(&workspace_id, LIMITE_MEMORIA_ATHENA)
        .await
}

/// Una persona responde por un recuerdo.
///
/// Es el único camino a `user_confirmed`: ningún módulo del runtime puede alcanzarlo, y
/// Athena tiene una prueba que lo prohíbe. La interfaz **no** confirma nada por su
/// cuenta — una propuesta que se convirtiera sola en hecho haría que el escalón más alto
/// de la memoria significara lo mismo que el más bajo.
#[tauri::command]
pub(crate) async fn confirm_athena_memory(
    memory_id: String,
    state: State<'_, AppState>,
) -> Result<athena::RecuerdoProyecto, AppError> {
    state.athena.cliente().confirmar_recuerdo(&memory_id).await
}

/// Retira un recuerdo. No lo borra: queda constancia de que se creyó.
#[tauri::command]
pub(crate) async fn forget_athena_memory(
    memory_id: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    if !confirmed {
        return Err(AppError::Validation(
            "hace falta confirmar antes de olvidar un recuerdo".to_owned(),
        ));
    }
    state.athena.cliente().olvidar_recuerdo(&memory_id).await
}

/// Los perfiles que ofrece este Athena, y cuál usa si no se pide ninguno.
///
/// La lista viene del servicio y no de una copia local: un perfil cambia qué
/// herramientas existen y qué cuenta como prueba, y una lista escrita aquí caducaría en
/// silencio en cuanto el despliegue añadiera uno.
/// Los modelos entre los que este despliegue deja elegir.
///
/// Se pregunta en vez de escribirse aquí por el mismo motivo que los perfiles: la lista
/// la decide quien despliega Athena, y una copia en el cliente ofrecería los modelos de
/// otro Athena. Un despliegue sin elección devuelve la lista vacía y no un error.
#[tauri::command]
pub(crate) async fn list_athena_models(
    state: State<'_, AppState>,
) -> Result<athena::ListadoModelos, AppError> {
    state.athena.cliente().listar_modelos().await
}

#[tauri::command]
pub(crate) async fn list_athena_profiles(
    state: State<'_, AppState>,
) -> Result<athena::ListadoPerfiles, AppError> {
    state.athena.cliente().listar_perfiles().await
}

/// Encargo vigente de un run, con su número de revisión.
///
/// Se pide aparte porque la instantánea no lo lleva. La interfaz lo necesita antes de
/// ofrecer un cambio: sin revisión no hay sobre qué decir que se escribe.
#[tauri::command]
pub(crate) async fn get_athena_goal(
    run_id: String,
    state: State<'_, AppState>,
) -> Result<athena::ObjetivoRun, AppError> {
    state.athena.objetivo(&run_id).await
}

/// Cambia el encargo de un run vivo sobre la revisión que esta aplicación conoce.
///
/// El número de revisión **no viaja desde React**: lo pone el núcleo, que es quien lo
/// mantiene al día con los eventos. Aceptarlo de la interfaz permitiría escribir sobre
/// una revisión que la persona nunca llegó a ver.
///
/// Un conflicto vuelve como resultado, no como error: la interfaz recibe el encargo
/// vigente y decide, y nada se reintenta solo.
#[tauri::command]
pub(crate) async fn revise_athena_goal(
    run_id: String,
    objective: String,
    reason: String,
    state: State<'_, AppState>,
) -> Result<athena::RevisionObjetivo, AppError> {
    state
        .athena
        .revisar_objetivo(&run_id, &objective, &reason)
        .await
}

/// Runs que quedaron a medias cuando el runtime murió.
#[tauri::command]
pub(crate) async fn list_athena_recovery_runs(
    state: State<'_, AppState>,
) -> Result<Vec<athena::ResumenRun>, AppError> {
    state.athena.cliente().runs_por_recuperar().await
}

#[tauri::command]
pub(crate) async fn cancel_athena_run(
    run_id: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    state.athena.cliente().cancelar_run(&run_id).await
}

/// Reanuda un run interrumpido y vuelve a seguirlo.
#[tauri::command]
pub(crate) async fn resume_athena_run(
    run_id: String,
    folder_id: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let carpeta = state
        .database
        .list_authorized_folders()?
        .into_iter()
        .find(|folder| folder.id == folder_id)
        .ok_or_else(|| AppError::Validation("la carpeta no está autorizada".to_owned()))?;
    state
        .athena
        .cliente()
        .reanudar_run(&run_id, &carpeta.path)
        .await?;
    state.athena.seguir(&run_id);
    Ok(())
}

/// Responde a una petición de permiso.
///
/// El acuse va antes que la decisión a propósito: es lo que detiene el reloj
/// corto de entrega y arranca el largo, de modo que una red lenta no se coma el
/// tiempo de pensar de la persona.
/// Avisa a Athena de que la pregunta ya está delante de la persona.
///
/// Athena mide con tres relojes (ADR-017): uno corto de entrega —«¿ha llegado esto a una
/// pantalla?»— y uno largo de decisión, que **sólo arranca con este aviso**. Sin él, el
/// corto vence a los 30 s y la respuesta se da por no llegada aunque haya alguien
/// leyéndola: se vio en un run real, donde cinco permisos murieron exactamente a los 30,0
/// segundos porque este aviso se mandaba al *responder* y no al *mostrar*, así que el
/// reloj de pensar no llegaba a empezar nunca.
///
/// Un fallo aquí no se propaga: no haber podido avisar no es motivo para no enseñar la
/// pregunta, y lo peor que pasa es que se vuelva a la conducta anterior.
#[tauri::command]
pub(crate) async fn acknowledge_athena_permission(
    run_id: String,
    request_id: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let Some(suscriptor) = state
        .athena
        .proyeccion(&run_id)
        .and_then(|vista| vista.suscriptor)
    else {
        // Todavía no hay conexión con el run. No es un error que enseñar: el aviso se
        // repetirá en cuanto la haya.
        return Ok(());
    };
    let _ = state
        .athena
        .cliente()
        .confirmar_recepcion_permiso(&run_id, &request_id, &suscriptor)
        .await;
    Ok(())
}

#[tauri::command]
pub(crate) async fn resolve_athena_permission(
    run_id: String,
    request_id: String,
    allow: bool,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    // La petición se lee antes de nada: da la herramienta y la acción que se
    // auditan, y su ausencia ya es la respuesta a tres de los casos difíciles
    // —caducada, run cancelado, o contestada un momento antes—.
    let permiso = state.athena.permiso(&run_id, &request_id);
    let (herramienta, accion) = permiso
        .as_ref()
        .map(|p| (p.herramienta.clone(), p.accion.clone()))
        .unwrap_or_default();
    let auditar = |resultado: &str| {
        state.database.record_athena_permission_decision(
            &run_id,
            &request_id,
            &herramienta,
            &accion,
            allow,
            resultado,
        )
    };

    match permiso.as_ref() {
        None => {
            auditar("retirada")?;
            return Err(AppError::AthenaRequestGone);
        }
        Some(pendiente) if pendiente.caducado => {
            state.athena.retirar_permiso(&run_id, &request_id);
            auditar("caducada")?;
            return Err(AppError::AthenaRequestGone);
        }
        Some(_) => {}
    }

    let suscriptor = state
        .athena
        .proyeccion(&run_id)
        .and_then(|vista| vista.suscriptor)
        .ok_or_else(|| {
            AppError::Conflict(
                "todavía no hay conexión con el run; no se puede responder al permiso".to_owned(),
            )
        })?;
    // Se retira ya: entre el envío y la vuelta hay tiempo de sobra para un
    // segundo clic, y esa segunda respuesta no debe salir.
    state.athena.retirar_permiso(&run_id, &request_id);

    let cliente = state.athena.cliente();
    let _ = cliente
        .confirmar_recepcion_permiso(&run_id, &request_id, &suscriptor)
        .await;
    let decision = if allow {
        athena::DecisionPermiso::Permitir
    } else {
        athena::DecisionPermiso::Denegar
    };
    let resultado = cliente
        .resolver_permiso(&run_id, &request_id, decision, &suscriptor)
        .await;
    // Se audita la decisión de la persona, haya llegado a aplicarse o no: lo
    // que importa del registro es quién dijo qué, no si el reloj lo permitió.
    match &resultado {
        Ok(()) => auditar("aplicada")?,
        Err(AppError::AthenaAlreadyResolved) => auditar("ya_resuelta")?,
        Err(AppError::AthenaRequestGone) => auditar("caducada")?,
        Err(_) => auditar("error_de_transporte")?,
    }
    resultado
}

/// Descarga un resultado externalizado por su clave.
#[tauri::command]
pub(crate) async fn fetch_athena_artifact(
    store_key: String,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    state.athena.cliente().descargar_artefacto(&store_key).await
}

pub(crate) fn validated_text(value: &str, field: &str, maximum: usize) -> Result<String, AppError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::Validation(format!(
            "{field} no puede estar vacío"
        )));
    }
    if value.chars().count() > maximum {
        return Err(AppError::Validation(format!(
            "{field} supera el límite de {maximum} caracteres"
        )));
    }
    Ok(value.to_owned())
}
