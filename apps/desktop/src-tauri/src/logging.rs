//! Registro estructurado local con correlación y redacción por construcción.
//!
//! La Fase 0 exige un sistema de logs observable que nunca contenga secretos ni
//! datos personales innecesarios. En lugar de aceptar texto libre y depender de
//! filtros por nombre de clave, este módulo solo admite un vocabulario acotado
//! de valores: recuentos, banderas, identificadores, códigos controlados y
//! duraciones. Así ningún prompt, ruta, título ni token puede llegar al archivo
//! aunque quien instrumente el código se equivoque: el valor que no cumple el
//! formato se sustituye por `[redactado]` en lugar de escribirse.
//!
//! Cada línea es un objeto JSON independiente (JSONL) en
//! `<data_dir>/logs/chatygpt.log`, con rotación por tamaño y una única copia
//! previa, de modo que el registro no puede crecer sin límite.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use uuid::Uuid;

use crate::error::AppError;

/// Tamaño máximo del registro activo antes de rotar.
const MAX_LOG_BYTES: u64 = 1_048_576;
/// Longitud máxima de un identificador aceptado sin redactar.
const MAX_ID_CHARS: usize = 64;
/// Longitud máxima de un código de vocabulario controlado.
///
/// Deliberadamente corta: los códigos reales del dominio (`waiting_for_tools`,
/// `broker_response`) caben de sobra, mientras que un secreto o un fragmento de
/// texto pegado por error casi siempre la supera y acaba redactado.
const MAX_CODE_CHARS: usize = 32;
/// Número máximo de campos conservados por evento.
const MAX_FIELDS: usize = 16;

const REDACTED: &str = "[redactado]";

/// Severidad del evento registrado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl Level {
    fn as_str(self) -> &'static str {
        match self {
            Level::Info => "info",
            Level::Warn => "warn",
            Level::Error => "error",
        }
    }
}

/// Único tipo de valor admitido dentro de un evento.
///
/// No existe una variante de texto libre: es la garantía estructural de que el
/// registro no puede contener contenido de conversaciones, rutas ni secretos.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    /// Magnitud entera: intentos, elementos recuperados, bytes, códigos HTTP.
    Count(i64),
    /// Bandera booleana.
    Flag(bool),
    /// Identificador interno (UUID o clave corta sin espacios).
    Id(String),
    /// Código de vocabulario controlado: estado, clase de error, modo.
    Code(String),
    /// Duración en milisegundos.
    Millis(u128),
}

/// Recuento o magnitud entera.
pub fn count(value: i64) -> FieldValue {
    FieldValue::Count(value)
}

/// Bandera booleana.
pub fn flag(value: bool) -> FieldValue {
    FieldValue::Flag(value)
}

/// Identificador interno; se redacta si no es una clave corta segura.
pub fn id(value: &str) -> FieldValue {
    FieldValue::Id(value.to_owned())
}

/// Código controlado; se redacta si contiene espacios o supera el límite.
pub fn code(value: &str) -> FieldValue {
    FieldValue::Code(value.to_owned())
}

/// Duración en milisegundos.
pub fn millis(value: u128) -> FieldValue {
    FieldValue::Millis(value)
}

/// Genera un identificador de correlación para una operación local.
pub fn new_correlation_id() -> String {
    Uuid::new_v4().simple().to_string()
}

/// Destino físico del registro, con rotación acotada.
struct LogSink {
    active_path: PathBuf,
    previous_path: PathBuf,
    max_bytes: u64,
}

impl LogSink {
    fn new(directory: &Path, max_bytes: u64) -> Result<Self, AppError> {
        fs::create_dir_all(directory)
            .map_err(|error| AppError::DataDirectory(error.to_string()))?;
        Ok(Self {
            active_path: directory.join("chatygpt.log"),
            previous_path: directory.join("chatygpt.log.1"),
            max_bytes,
        })
    }

    /// Escribe una línea ya renderizada, rotando antes si el archivo está lleno.
    ///
    /// Un fallo de escritura nunca interrumpe la operación en curso: el registro
    /// es observabilidad, no una fuente de verdad.
    fn write_line(&self, line: &str) -> std::io::Result<()> {
        let current = fs::metadata(&self.active_path)
            .map(|data| data.len())
            .unwrap_or(0);
        if current > 0 && current + line.len() as u64 + 1 > self.max_bytes {
            let _ = fs::remove_file(&self.previous_path);
            fs::rename(&self.active_path, &self.previous_path)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.active_path)?;
        writeln!(file, "{line}")
    }
}

static SINK: OnceLock<Mutex<LogSink>> = OnceLock::new();

/// Prepara el registro dentro del directorio de datos de la aplicación.
///
/// Es idempotente: una segunda llamada conserva el destino ya configurado.
pub fn init(data_dir: &Path) -> Result<PathBuf, AppError> {
    let sink = LogSink::new(&data_dir.join("logs"), MAX_LOG_BYTES)?;
    let path = sink.active_path.clone();
    let _ = SINK.set(Mutex::new(sink));
    Ok(SINK
        .get()
        .and_then(|sink| sink.lock().ok().map(|sink| sink.active_path.clone()))
        .unwrap_or(path))
}

/// Ruta del registro activo, cuando ya se ha inicializado.
pub fn log_path() -> Option<PathBuf> {
    SINK.get()
        .and_then(|sink| sink.lock().ok().map(|sink| sink.active_path.clone()))
}

/// Registra un evento. Antes de la inicialización, se descarta en silencio.
pub fn record(level: Level, event: &str, correlation: Option<&str>, fields: &[(&str, FieldValue)]) {
    let Some(sink) = SINK.get() else {
        return;
    };
    let line = render_event(level, event, correlation, fields, SystemTime::now());
    if let Ok(sink) = sink.lock() {
        let _ = sink.write_line(&line);
    }
}

/// Evento informativo del ciclo de vida normal.
pub fn info(event: &str, correlation: Option<&str>, fields: &[(&str, FieldValue)]) {
    record(Level::Info, event, correlation, fields);
}

/// Situación anómala recuperable.
pub fn warn(event: &str, correlation: Option<&str>, fields: &[(&str, FieldValue)]) {
    record(Level::Warn, event, correlation, fields);
}

/// Fallo que impide completar la operación.
pub fn error(event: &str, correlation: Option<&str>, fields: &[(&str, FieldValue)]) {
    record(Level::Error, event, correlation, fields);
}

/// Clase estable de un error, apta para el registro y para métricas.
///
/// Se registra la clase y nunca el mensaje: el texto de un error del Broker
/// puede arrastrar fragmentos del contenido enviado.
pub fn error_kind(error: &AppError) -> FieldValue {
    let kind = match error {
        AppError::DataDirectory(_) => "data_directory",
        AppError::Database(_) => "database",
        AppError::InvalidBrokerUrl(_) => "invalid_broker_url",
        AppError::BrokerTransport(_) => "broker_transport",
        AppError::BrokerResponse { .. } => "broker_response",
        AppError::BrokerContract(_) => "broker_contract",
        AppError::Validation(_) => "validation",
        AppError::NotFound(_) => "not_found",
        AppError::Conflict(_) => "conflict",
    };
    FieldValue::Code(kind.to_owned())
}

/// Renderiza un evento como una línea JSON con marca de tiempo UTC.
fn render_event(
    level: Level,
    event: &str,
    correlation: Option<&str>,
    fields: &[(&str, FieldValue)],
    now: SystemTime,
) -> String {
    let mut line = String::with_capacity(160);
    line.push('{');
    push_pair(&mut line, "ts", &json_string(&iso8601_utc(now)));
    line.push(',');
    push_pair(&mut line, "level", &json_string(level.as_str()));
    line.push(',');
    push_pair(&mut line, "event", &json_string(&safe_code(event)));
    line.push(',');
    push_pair(
        &mut line,
        "app_version",
        &json_string(env!("CARGO_PKG_VERSION")),
    );
    if let Some(correlation) = correlation {
        line.push(',');
        push_pair(
            &mut line,
            "correlation_id",
            &json_string(&safe_id(correlation)),
        );
    }
    for (key, value) in fields.iter().take(MAX_FIELDS) {
        line.push(',');
        push_pair(&mut line, &safe_code(key), &render_value(value));
    }
    line.push('}');
    line
}

fn push_pair(target: &mut String, key: &str, rendered_value: &str) {
    target.push_str(&json_string(key));
    target.push(':');
    target.push_str(rendered_value);
}

fn render_value(value: &FieldValue) -> String {
    match value {
        FieldValue::Count(value) => value.to_string(),
        FieldValue::Flag(value) => value.to_string(),
        FieldValue::Millis(value) => value.to_string(),
        FieldValue::Id(value) => json_string(&safe_id(value)),
        FieldValue::Code(value) => json_string(&safe_code(value)),
    }
}

/// Acepta identificadores internos; cualquier otra cosa se redacta.
fn safe_id(value: &str) -> String {
    let acceptable = !value.is_empty()
        && value.chars().count() <= MAX_ID_CHARS
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'));
    if acceptable {
        value.to_owned()
    } else {
        REDACTED.to_owned()
    }
}

/// Acepta códigos de vocabulario controlado; cualquier otra cosa se redacta.
///
/// El vocabulario del dominio es `snake_case` en minúsculas, por lo que exigir
/// minúsculas descarta de paso buena parte de los secretos y del texto humano.
fn safe_code(value: &str) -> String {
    let acceptable = !value.is_empty()
        && value.chars().count() <= MAX_CODE_CHARS
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '-' | '_' | '.' | ':')
        });
    if acceptable {
        value.to_owned()
    } else {
        REDACTED.to_owned()
    }
}

/// Serializa una cadena como literal JSON con escapes mínimos.
fn json_string(value: &str) -> String {
    let mut rendered = String::with_capacity(value.len() + 2);
    rendered.push('"');
    for character in value.chars() {
        match character {
            '"' => rendered.push_str("\\\""),
            '\\' => rendered.push_str("\\\\"),
            '\n' => rendered.push_str("\\n"),
            '\r' => rendered.push_str("\\r"),
            '\t' => rendered.push_str("\\t"),
            control if (control as u32) < 0x20 => {
                rendered.push_str(&format!("\\u{:04x}", control as u32));
            }
            other => rendered.push(other),
        }
    }
    rendered.push('"');
    rendered
}

/// Marca de tiempo UTC en formato ISO-8601 con milisegundos.
fn iso8601_utc(now: SystemTime) -> String {
    let elapsed = now.duration_since(UNIX_EPOCH).unwrap_or_default();
    let seconds = elapsed.as_secs() as i64;
    let millis = elapsed.subsec_millis();
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        seconds_of_day / 3_600,
        (seconds_of_day % 3_600) / 60,
        seconds_of_day % 60
    )
}

/// Conversión de días desde epoch a fecha civil (algoritmo de Howard Hinnant).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_position = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_position + 2) / 5 + 1) as u32;
    let month = if month_position < 10 {
        month_position + 3
    } else {
        month_position - 9
    } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn temporary_directory(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!("chatygpt-log-{label}-{}", Uuid::new_v4().simple()))
    }

    fn at_epoch_millis(value: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_millis(value)
    }

    #[test]
    fn secrets_paths_and_free_text_can_never_reach_the_log() {
        let token = "sk-live-4f9a2c7d1e8b6a3f5c0d9e2b7a4f1c8d";
        let line = render_event(
            Level::Error,
            "broker.request_failed",
            Some("9f4c1d2e3a4b5c6d7e8f90a1b2c3d4e5"),
            &[
                // Un descuido al instrumentar: valores que jamás deben persistir.
                ("token", code(token)),
                (
                    "path",
                    code(r"C:\Users\jfeli\Documentos\informe médico.pdf"),
                ),
                ("prompt", code("Resume mi informe médico y mis apellidos")),
                ("detail", id("Broker AI devolvió HTTP 422: campo inválido")),
                // Valores legítimos que sí deben conservarse.
                ("status", count(422)),
                ("attempt", count(2)),
                ("error_kind", code("broker_response")),
            ],
            at_epoch_millis(1_800_000_000_000),
        );

        assert!(!line.contains(token), "el token no puede aparecer: {line}");
        assert!(!line.contains("jfeli"), "la ruta no puede aparecer: {line}");
        assert!(
            !line.contains("médico"),
            "el prompt no puede aparecer: {line}"
        );
        assert!(
            !line.contains("campo inválido"),
            "el mensaje del Broker no puede aparecer: {line}"
        );
        assert_eq!(line.matches(REDACTED).count(), 4);
        assert!(line.contains(r#""status":422"#));
        assert!(line.contains(r#""attempt":2"#));
        assert!(line.contains(r#""error_kind":"broker_response""#));
        assert!(line.contains(r#""correlation_id":"9f4c1d2e3a4b5c6d7e8f90a1b2c3d4e5""#));
        assert!(line.contains(r#""level":"error""#));
    }

    #[test]
    fn every_line_is_valid_json_with_utc_timestamp_and_version() {
        let line = render_event(
            Level::Info,
            "task.submitted",
            Some("0f1e2d3c"),
            &[
                ("state", code("polling")),
                ("recovered", flag(false)),
                ("latency_ms", millis(1_234)),
            ],
            at_epoch_millis(1_800_000_000_500),
        );

        let parsed: serde_json::Value =
            serde_json::from_str(&line).expect("cada línea debe ser JSON válido");
        assert_eq!(parsed["ts"], "2027-01-15T08:00:00.500Z");
        assert_eq!(parsed["event"], "task.submitted");
        assert_eq!(parsed["app_version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(parsed["state"], "polling");
        assert_eq!(parsed["recovered"], false);
        assert_eq!(parsed["latency_ms"], 1_234);
    }

    #[test]
    fn timestamps_cover_leap_years_and_epoch_boundaries() {
        assert!(iso8601_utc(at_epoch_millis(0)).starts_with("1970-01-01T00:00:00.000Z"));
        // 2024-02-29T23:59:59Z, para comprobar el año bisiesto.
        assert_eq!(
            iso8601_utc(at_epoch_millis(1_709_251_199_000)),
            "2024-02-29T23:59:59.000Z"
        );
        // 2000-03-01T00:00:00Z, frontera del siglo bisiesto.
        assert_eq!(
            iso8601_utc(at_epoch_millis(951_868_800_000)),
            "2000-03-01T00:00:00.000Z"
        );
    }

    #[test]
    fn identifiers_survive_but_arbitrary_text_does_not() {
        let uuid = Uuid::new_v4().to_string();
        assert_eq!(safe_id(&uuid), uuid);
        assert_eq!(safe_id("local-task-42"), "local-task-42");
        assert_eq!(safe_id("con espacios"), REDACTED);
        assert_eq!(safe_id(&"a".repeat(MAX_ID_CHARS + 1)), REDACTED);
        assert_eq!(safe_id(""), REDACTED);
        assert_eq!(safe_code("waiting_for_tools"), "waiting_for_tools");
        assert_eq!(safe_code("Resume mi informe"), REDACTED);
        assert_eq!(safe_code(&"a".repeat(MAX_CODE_CHARS + 1)), REDACTED);
        assert_eq!(safe_code("Bearer-XYZ"), REDACTED);
    }

    #[test]
    fn the_log_rotates_by_size_and_keeps_a_single_previous_copy() {
        let directory = temporary_directory("rotation");
        let sink = LogSink::new(&directory, 256).expect("el destino debe crearse");
        let line = "x".repeat(100);

        for _ in 0..8 {
            sink.write_line(&line).expect("la escritura debe funcionar");
        }

        let active =
            fs::read_to_string(&sink.active_path).expect("debe existir el registro activo");
        let previous =
            fs::read_to_string(&sink.previous_path).expect("debe existir una copia previa");
        assert!(active.len() as u64 <= 256);
        assert!(previous.len() as u64 <= 256);
        assert!(!directory.join("chatygpt.log.2").exists());
        fs::remove_dir_all(directory).expect("el directorio de prueba debe borrarse");
    }

    #[test]
    fn error_classes_are_recorded_without_their_message() {
        let error = AppError::BrokerResponse {
            status: 422,
            message: "el prompt contiene datos personales".to_owned(),
        };
        assert_eq!(
            error_kind(&error),
            FieldValue::Code("broker_response".into())
        );

        let line = render_event(
            Level::Error,
            "task.failed",
            None,
            &[("error_kind", error_kind(&error))],
            at_epoch_millis(1_800_000_000_000),
        );
        assert!(!line.contains("datos personales"));
        assert!(line.contains(r#""error_kind":"broker_response""#));
    }

    #[test]
    fn a_ready_sink_writes_one_json_line_per_event() {
        let directory = temporary_directory("sink");
        let sink = LogSink::new(&directory, MAX_LOG_BYTES).expect("el destino debe crearse");
        sink.write_line(&render_event(
            Level::Info,
            "app.started",
            None,
            &[("schema_version", count(15))],
            at_epoch_millis(1_800_000_000_000),
        ))
        .expect("la escritura debe funcionar");

        let content = fs::read_to_string(&sink.active_path).expect("el registro debe leerse");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 1);
        let parsed: serde_json::Value =
            serde_json::from_str(lines[0]).expect("la línea debe ser JSON válido");
        assert_eq!(parsed["event"], "app.started");
        assert_eq!(parsed["schema_version"], 15);
        fs::remove_dir_all(directory).expect("el directorio de prueba debe borrarse");
    }
}
