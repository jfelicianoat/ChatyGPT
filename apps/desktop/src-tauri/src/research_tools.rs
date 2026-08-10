//! Herramientas que ChatyGPT ejecuta durante una investigación.
//!
//! Esto es código que corre **en el equipo de la persona a petición de un
//! modelo**. Esa frase es todo el diseño del módulo: la URL no la escribe
//! nadie de confianza, la elige un modelo a partir de lo que ha leído en la
//! web, así que cada límite de aquí existe para acotar lo que puede pasar.
//!
//! La validación está separada de la descarga a propósito: decidir si una URL
//! es admisible es una función pura que puede probarse exhaustivamente sin
//! red, y es donde vive la seguridad.

use std::net::IpAddr;

use reqwest::Client;
use serde::Serialize;
use url::{Host, Url};

use crate::error::AppError;

/// Tamaño máximo que se descarga de una página.
///
/// Dos megabytes son de sobra para el texto de un artículo y evitan que una
/// respuesta enorme llene la memoria o el contexto del modelo.
pub const MAX_FETCH_BYTES: usize = 2 * 1024 * 1024;

/// Caracteres de texto que se devuelven al modelo.
///
/// El resultado viaja de vuelta al Broker en `tool_results`, cuyo contrato
/// limita el contenido a 200.000 caracteres. Se recorta antes para no depender
/// de que el otro lado rechace la petición.
pub const MAX_FETCH_CHARACTERS: usize = 40_000;

/// Segundos que se espera a una página antes de darla por perdida.
const FETCH_TIMEOUT_SECONDS: u64 = 15;

/// Redirecciones seguidas antes de rendirse.
const MAX_REDIRECTS: usize = 3;

/// Página descargada y reducida a texto.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchedPage {
    /// URL final, después de redirecciones.
    pub url: String,
    pub title: Option<String>,
    pub text: String,
    pub truncated: bool,
}

/// Decide si una URL puede abrirse, y la normaliza.
///
/// Rechaza, por este orden y con motivos distintos:
///
/// - lo que no es una URL absoluta;
/// - esquemas que no son web —`file://` leería el disco de la persona—;
/// - URLs con credenciales incrustadas, que enviarían un secreto a un tercero;
/// - direcciones de bucle local y redes privadas, que apuntarían al propio
///   equipo o a la red doméstica: ahí viven el Broker y su token, y un modelo
///   no tiene por qué poder llamar a la puerta.
pub fn validate_fetch_url(raw: &str) -> Result<Url, AppError> {
    let url = Url::parse(raw.trim())
        .map_err(|_| AppError::Validation("la URL indicada no es válida".to_owned()))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(AppError::Validation(
            "solo se pueden abrir direcciones http o https".to_owned(),
        ));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(AppError::Validation(
            "no se abren URLs con credenciales incrustadas".to_owned(),
        ));
    }
    match url.host() {
        Some(Host::Domain(domain)) => {
            let domain = domain.to_ascii_lowercase();
            if domain == "localhost" || domain.ends_with(".localhost") {
                return Err(AppError::Validation(
                    "no se abren direcciones del propio equipo".to_owned(),
                ));
            }
        }
        Some(Host::Ipv4(address)) => reject_private_address(IpAddr::V4(address))?,
        Some(Host::Ipv6(address)) => reject_private_address(IpAddr::V6(address))?,
        None => {
            return Err(AppError::Validation(
                "la URL indicada no tiene servidor".to_owned(),
            ))
        }
    }
    Ok(url)
}

fn reject_private_address(address: IpAddr) -> Result<(), AppError> {
    let private = match address {
        IpAddr::V4(v4) => {
            v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified()
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                // Direcciones únicas locales (fc00::/7), el equivalente a las privadas de IPv4.
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                // Enlace local (fe80::/10).
                || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
    };
    if private {
        return Err(AppError::Validation(
            "no se abren direcciones del propio equipo ni de la red local".to_owned(),
        ));
    }
    Ok(())
}

/// Reduce un HTML a texto legible.
///
/// Es una extracción deliberadamente ingenua: quita guiones de comentario,
/// descarta por completo `script` y `style` —cuyo contenido no es prosa—,
/// elimina el resto de etiquetas y colapsa los espacios. No interpreta el
/// documento ni intenta adivinar el cuerpo del artículo. Para lo que se
/// necesita —darle al modelo el texto de una fuente para que pueda citarla—
/// basta, y evita traer un analizador de HTML entero.
pub fn extract_readable_text(html: &str, limit: usize) -> (String, bool) {
    let without_blocks = strip_blocks(html);
    let mut text = String::with_capacity(without_blocks.len());
    let mut inside_tag = false;
    for character in without_blocks.chars() {
        match character {
            '<' => inside_tag = true,
            '>' => {
                inside_tag = false;
                text.push(' ');
            }
            _ if !inside_tag => text.push(character),
            _ => {}
        }
    }
    let collapsed = collapse_whitespace(&decode_basic_entities(&text));
    let truncated = collapsed.chars().count() > limit;
    let text = if truncated {
        collapsed.chars().take(limit).collect()
    } else {
        collapsed
    };
    (text, truncated)
}

/// Título del documento, si lo declara.
pub fn extract_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let start = lower.find("<title")?;
    let open_end = lower[start..].find('>')? + start + 1;
    let end = lower[open_end..].find("</title>")? + open_end;
    let title = collapse_whitespace(&decode_basic_entities(&html[open_end..end]));
    if title.is_empty() {
        None
    } else {
        Some(title.chars().take(300).collect())
    }
}

fn strip_blocks(html: &str) -> String {
    let mut result = html.to_owned();
    for tag in ["script", "style", "noscript"] {
        result = strip_tag_blocks(&result, tag);
    }
    result
}

fn strip_tag_blocks(html: &str, tag: &str) -> String {
    let lower = html.to_lowercase();
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut result = String::with_capacity(html.len());
    let mut cursor = 0;
    while let Some(start) = lower[cursor..].find(&open) {
        let start = cursor + start;
        result.push_str(&html[cursor..start]);
        match lower[start..].find(&close) {
            Some(end) => cursor = start + end + close.len(),
            None => return result,
        }
    }
    result.push_str(&html[cursor..]);
    result
}

fn decode_basic_entities(text: &str) -> String {
    text.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Cliente HTTP para abrir páginas, separado del que habla con el Broker.
///
/// No lleva la credencial del Broker ni sus tiempos de espera: son destinos
/// distintos y mezclarlos arriesgaría enviar el token a un tercero.
pub fn web_client() -> Result<Client, AppError> {
    Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(FETCH_TIMEOUT_SECONDS))
        .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS))
        .user_agent(concat!("ChatyGPT/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| AppError::BrokerTransport(error.to_string()))
}

/// Abre una página y devuelve su texto.
pub async fn fetch_url(client: &Client, raw_url: &str) -> Result<FetchedPage, AppError> {
    let url = validate_fetch_url(raw_url)?;
    let response = client
        .get(url.clone())
        .send()
        .await
        .map_err(|error| AppError::BrokerTransport(error.to_string()))?;
    let final_url = response.url().to_string();
    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .map_err(|error| AppError::BrokerTransport(error.to_string()))?;
    if !status.is_success() {
        return Err(AppError::BrokerResponse {
            status: status.as_u16(),
            message: format!("la página respondió HTTP {}", status.as_u16()),
        });
    }
    if bytes.len() > MAX_FETCH_BYTES {
        return Err(AppError::Validation(format!(
            "la página supera el límite local de {} MB",
            MAX_FETCH_BYTES / (1024 * 1024)
        )));
    }
    let body = String::from_utf8_lossy(&bytes);
    let (text, truncated) = extract_readable_text(&body, MAX_FETCH_CHARACTERS);
    Ok(FetchedPage {
        url: final_url,
        title: extract_title(&body),
        text,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::{extract_readable_text, extract_title, validate_fetch_url, MAX_FETCH_CHARACTERS};
    use crate::error::AppError;

    #[test]
    fn only_web_addresses_are_opened() {
        assert!(validate_fetch_url("https://example.org/informe").is_ok());
        assert!(validate_fetch_url("http://example.org/informe").is_ok());
        // Un esquema de archivo leería el disco de la persona.
        assert!(matches!(
            validate_fetch_url("file:///C:/Windows/System32/config/SAM"),
            Err(AppError::Validation(_))
        ));
        assert!(validate_fetch_url("ftp://example.org/x").is_err());
        assert!(validate_fetch_url("javascript:alert(1)").is_err());
        // Una ruta relativa no es una URL absoluta.
        assert!(validate_fetch_url("/informe").is_err());
        assert!(validate_fetch_url("").is_err());
    }

    #[test]
    fn credentials_are_never_forwarded_to_a_third_party() {
        assert!(matches!(
            validate_fetch_url("https://usuario:secreto@example.org/x"),
            Err(AppError::Validation(_))
        ));
        assert!(validate_fetch_url("https://usuario@example.org/x").is_err());
    }

    #[test]
    fn the_local_machine_and_the_home_network_are_out_of_reach() {
        // Ahí viven el Broker y su token: un modelo no tiene por qué poder
        // llamar a esa puerta desde una URL que él mismo eligió.
        for address in [
            "http://127.0.0.1:8765/api/v1/tasks",
            "http://localhost:8765/",
            "http://LOCALHOST/",
            "http://algo.localhost/",
            "http://192.168.1.52:8765/",
            "http://10.0.0.5/",
            "http://172.16.3.4/",
            "http://169.254.169.254/latest/meta-data/",
            "http://0.0.0.0/",
            "http://[::1]/",
            "http://[fe80::1]/",
            "http://[fc00::1]/",
        ] {
            assert!(
                validate_fetch_url(address).is_err(),
                "debía rechazarse: {address}"
            );
        }
        // Una dirección pública sí se abre.
        assert!(validate_fetch_url("http://93.184.216.34/").is_ok());
    }

    #[test]
    fn script_and_style_contents_never_reach_the_model() {
        let html = "<html><head><style>body{color:red}</style>\
                    <script>var secreto = 1;</script></head>\
                    <body><h1>Informe</h1><p>Texto &amp; contenido</p></body></html>";
        let (text, truncated) = extract_readable_text(html, MAX_FETCH_CHARACTERS);
        assert!(text.contains("Informe"));
        assert!(text.contains("Texto & contenido"));
        assert!(!text.contains("color:red"));
        assert!(!text.contains("var secreto"));
        assert!(!truncated);
    }

    #[test]
    fn tags_are_removed_without_gluing_words_together() {
        // Sin separar por la etiqueta, «uno» y «dos» saldrían pegados y el
        // modelo citaría una palabra que no existe en la fuente.
        let (text, _) = extract_readable_text("<p>uno</p><p>dos</p>", MAX_FETCH_CHARACTERS);
        assert_eq!(text, "uno dos");
    }

    #[test]
    fn long_pages_are_cut_and_say_so() {
        let html = format!("<p>{}</p>", "palabra ".repeat(20_000));
        let (text, truncated) = extract_readable_text(&html, 100);
        assert_eq!(text.chars().count(), 100);
        assert!(
            truncated,
            "recortar en silencio ocultaría que falta contenido"
        );
    }

    #[test]
    fn an_unclosed_block_does_not_leak_its_contents() {
        // Un HTML roto no debe convertirse en una vía para colar el script.
        let (text, _) = extract_readable_text("<p>visible</p><script>oculto", MAX_FETCH_CHARACTERS);
        assert!(text.contains("visible"));
        assert!(!text.contains("oculto"));
    }

    #[test]
    fn the_title_is_read_when_the_page_declares_one() {
        assert_eq!(
            extract_title("<html><head><title> Informe  anual </title></head></html>").as_deref(),
            Some("Informe anual")
        );
        assert_eq!(
            extract_title("<title lang=\"es\">Con atributos</title>").as_deref(),
            Some("Con atributos")
        );
        assert!(extract_title("<html><body>sin título</body></html>").is_none());
        assert!(extract_title("<title></title>").is_none());
    }
}
