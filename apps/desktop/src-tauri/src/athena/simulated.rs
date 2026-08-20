//! Servicio de Athena simulado para pruebas de integración.
//!
//! Sigue el mismo criterio que el Broker simulado: los caminos que importan
//! —flujo SSE, reconexión, aprobación, artefacto expirado— son bucles que
//! reaccionan a códigos HTTP y a marcos de texto, y ninguna prueba sobre
//! funciones puras demuestra que terminan donde deben.
//!
//! Sin dependencias nuevas: habla HTTP/1.1 sobre `TcpListener` en `127.0.0.1`
//! con puerto efímero. Lo que aporta frente a un Athena real son dos cosas que
//! las pruebas necesitan y el runtime no da a voluntad: **guiones
//! programables** —incluidos cortes de conexión a mitad de flujo— y un
//! **registro de peticiones** con el que comprobar que el cliente manda la
//! cabecera de control.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

/// Petición observada, para comprobar qué mandó el cliente.
#[derive(Debug, Clone)]
pub struct PeticionVista {
    pub metodo: String,
    pub ruta: String,
    pub autorizacion: Option<String>,
    pub suscriptor: Option<String>,
    pub cuerpo: String,
}

/// Guion de una respuesta corriente.
#[derive(Debug, Clone)]
pub struct RespuestaGuion {
    pub estado: u16,
    pub cuerpo: Vec<u8>,
    pub tipo: &'static str,
}

impl RespuestaGuion {
    pub fn ok(cuerpo: Value) -> Self {
        Self {
            estado: 200,
            cuerpo: cuerpo.to_string().into_bytes(),
            tipo: "application/json",
        }
    }

    pub fn creado(cuerpo: Value) -> Self {
        Self {
            estado: 201,
            cuerpo: cuerpo.to_string().into_bytes(),
            tipo: "application/json",
        }
    }

    pub fn texto(cuerpo: &str) -> Self {
        Self {
            estado: 200,
            cuerpo: cuerpo.as_bytes().to_vec(),
            tipo: "text/plain; charset=utf-8",
        }
    }

    pub fn error(estado: u16, codigo: &str, mensaje: &str) -> Self {
        Self {
            estado,
            cuerpo: json!({"error": {"code": codigo, "message": mensaje}})
                .to_string()
                .into_bytes(),
            tipo: "application/json",
        }
    }
}

/// Guion del flujo de eventos: marcos a emitir y si cortar al terminar.
#[derive(Debug, Clone, Default)]
pub struct GuionFlujo {
    pub marcos: Vec<String>,
    /// Cierra la conexión tras los marcos, para ejercitar la reconexión.
    pub cortar_al_final: bool,
    /// Retardo entre marcos, para que el cliente tenga tiempo de reaccionar.
    pub retardo: Option<Duration>,
}

impl GuionFlujo {
    pub fn marco_estado(suscriptor: &str, controla: bool, instantanea: Value) -> String {
        let datos = json!({
            "subscriber_id": suscriptor,
            "controls": controla,
            "wire_version": 1,
            "snapshot": instantanea,
            "pending_approvals": [],
        });
        format!("event: state\ndata: {datos}\n\n")
    }

    pub fn marco_evento(nombre: &str, run_id: &str, carga: Value) -> String {
        let datos = json!({
            "event_id": format!("ev-{nombre}"),
            "name": nombre,
            "run_id": run_id,
            "correlation_id": Value::Null,
            "occurred_at": "2026-08-19T00:00:00+00:00",
            "payload": carga,
        });
        format!("id: ev-{nombre}\nevent: event\ndata: {datos}\n\n")
    }
}

#[derive(Default)]
struct Estado {
    respuestas: VecDeque<(String, RespuestaGuion)>,
    flujos: VecDeque<GuionFlujo>,
    vistas: Vec<PeticionVista>,
}

/// Servicio simulado en un hilo propio.
pub struct AthenaSimulado {
    puerto: u16,
    estado: Arc<Mutex<Estado>>,
    detener: Arc<AtomicBool>,
}

impl AthenaSimulado {
    pub fn arrancar() -> Self {
        let escucha = TcpListener::bind("127.0.0.1:0").expect("no se pudo abrir el puerto");
        let puerto = escucha.local_addr().expect("sin dirección").port();
        let estado = Arc::new(Mutex::new(Estado::default()));
        let detener = Arc::new(AtomicBool::new(false));

        let estado_hilo = Arc::clone(&estado);
        let detener_hilo = Arc::clone(&detener);
        thread::spawn(move || {
            escucha
                .set_nonblocking(true)
                .expect("no se pudo poner en no bloqueante");
            while !detener_hilo.load(Ordering::SeqCst) {
                match escucha.accept() {
                    Ok((flujo, _)) => {
                        // En Windows el socket aceptado hereda el modo no
                        // bloqueante del listener: sin esto las lecturas fallan
                        // de forma intermitente.
                        flujo
                            .set_nonblocking(false)
                            .expect("no se pudo poner en bloqueante");
                        atender(flujo, &estado_hilo);
                    }
                    Err(ref error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            puerto,
            estado,
            detener,
        }
    }

    pub fn url_base(&self) -> String {
        format!("http://127.0.0.1:{}", self.puerto)
    }

    /// Programa la siguiente respuesta para las rutas que contengan `patron`.
    pub fn responder(&self, patron: &str, respuesta: RespuestaGuion) {
        self.estado
            .lock()
            .expect("estado envenenado")
            .respuestas
            .push_back((patron.to_owned(), respuesta));
    }

    /// Programa el siguiente flujo de eventos.
    pub fn emitir(&self, guion: GuionFlujo) {
        self.estado
            .lock()
            .expect("estado envenenado")
            .flujos
            .push_back(guion);
    }

    pub fn peticiones(&self) -> Vec<PeticionVista> {
        self.estado
            .lock()
            .expect("estado envenenado")
            .vistas
            .clone()
    }
}

impl Drop for AthenaSimulado {
    fn drop(&mut self) {
        self.detener.store(true, Ordering::SeqCst);
    }
}

fn atender(mut flujo: TcpStream, estado: &Arc<Mutex<Estado>>) {
    let mut lector = BufReader::new(flujo.try_clone().expect("no se pudo clonar el socket"));
    let mut linea = String::new();
    if lector.read_line(&mut linea).is_err() || linea.trim().is_empty() {
        return;
    }
    let mut partes = linea.split_whitespace();
    let metodo = partes.next().unwrap_or_default().to_owned();
    let ruta = partes.next().unwrap_or_default().to_owned();

    let mut longitud = 0usize;
    let mut autorizacion = None;
    let mut suscriptor = None;
    loop {
        let mut cabecera = String::new();
        if lector.read_line(&mut cabecera).is_err() {
            return;
        }
        let recortada = cabecera.trim_end();
        if recortada.is_empty() {
            break;
        }
        let (nombre, valor) = recortada.split_once(':').unwrap_or((recortada, ""));
        match nombre.to_ascii_lowercase().as_str() {
            "content-length" => longitud = valor.trim().parse().unwrap_or(0),
            "authorization" => autorizacion = Some(valor.trim().to_owned()),
            "x-athena-subscriber" => suscriptor = Some(valor.trim().to_owned()),
            _ => {}
        }
    }
    let mut cuerpo = vec![0u8; longitud];
    if longitud > 0 && lector.read_exact(&mut cuerpo).is_err() {
        return;
    }

    let vista = PeticionVista {
        metodo: metodo.clone(),
        ruta: ruta.clone(),
        autorizacion,
        suscriptor,
        cuerpo: String::from_utf8_lossy(&cuerpo).into_owned(),
    };

    let guion_flujo = {
        let mut guardado = estado.lock().expect("estado envenenado");
        guardado.vistas.push(vista);
        if ruta.contains("/events") {
            guardado.flujos.pop_front()
        } else {
            None
        }
    };

    if let Some(guion) = guion_flujo {
        responder_flujo(&mut flujo, &guion);
        return;
    }

    let respuesta = {
        let mut guardado = estado.lock().expect("estado envenenado");
        let posicion = guardado
            .respuestas
            .iter()
            .position(|(patron, _)| ruta.contains(patron.as_str()));
        match posicion {
            Some(indice) => guardado.respuestas.remove(indice).map(|(_, r)| r),
            None => None,
        }
    };

    let respuesta = respuesta
        .unwrap_or_else(|| RespuestaGuion::error(404, "not_found", "sin guion para esta ruta"));
    let cabecera = format!(
        "HTTP/1.1 {} X\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        respuesta.estado,
        respuesta.tipo,
        respuesta.cuerpo.len()
    );
    let _ = flujo.write_all(cabecera.as_bytes());
    let _ = flujo.write_all(&respuesta.cuerpo);
    let _ = flujo.flush();
}

fn responder_flujo(flujo: &mut TcpStream, guion: &GuionFlujo) {
    let cabecera = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\
                    Cache-Control: no-store\r\nConnection: close\r\n\r\n";
    if flujo.write_all(cabecera.as_bytes()).is_err() {
        return;
    }
    for marco in &guion.marcos {
        if flujo.write_all(marco.as_bytes()).is_err() {
            return;
        }
        let _ = flujo.flush();
        if let Some(espera) = guion.retardo {
            thread::sleep(espera);
        }
    }
    if guion.cortar_al_final {
        let _ = flujo.shutdown(std::net::Shutdown::Both);
    }
}
