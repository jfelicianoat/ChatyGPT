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

use std::net::{IpAddr, SocketAddr, ToSocketAddrs};

use reqwest::{blocking::Client as BlockingClient, Client};
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

/// Respuesta textual y acotada de una API externa.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalApiResponse {
    pub url: String,
    pub status: u16,
    pub content_type: Option<String>,
    pub body: String,
    pub truncated: bool,
}

/// Las APIs que invoque un GPT se restringen a HTTPS. Es una capacidad distinta
/// de abrir una fuente pública durante una investigación y siempre requiere
/// confirmación humana.
pub fn validate_external_api_url(raw: &str) -> Result<Url, AppError> {
    let url = validate_fetch_url(raw)?;
    if url.scheme() != "https" {
        return Err(AppError::Validation(
            "las llamadas a APIs externas deben usar HTTPS".to_owned(),
        ));
    }
    if raw.trim().chars().count() > 2_048 {
        return Err(AppError::Validation(
            "la URL de la API supera 2048 caracteres".to_owned(),
        ));
    }
    Ok(url)
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
    if !is_public_address(address) {
        return Err(AppError::Validation(
            "no se abren direcciones del propio equipo ni de la red local".to_owned(),
        ));
    }
    Ok(())
}

/// Solo las direcciones globales son destinos admisibles.
///
/// `is_private` e `is_loopback` no bastan: dejan pasar la red compartida de
/// los operadores (100.64/10), los rangos de documentación y de pruebas,
/// difusión, multidifusión, la red «esta» (0/8) y las IPv6 que envuelven una
/// IPv4 interna (NAT64, 6to4, compatibles).
pub(crate) fn is_public_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => {
            let [a, b, c, _] = v4.octets();
            !(v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_multicast()
                || a == 0
                // Red compartida de operadores (CGNAT), 100.64.0.0/10.
                || (a == 100 && (64..128).contains(&b))
                // Asignaciones de protocolo del IETF, 192.0.0.0/24.
                || (a == 192 && b == 0 && c == 0)
                // Pruebas de rendimiento, 198.18.0.0/15.
                || (a == 198 && (b == 18 || b == 19))
                // Reservado para uso futuro, 240.0.0.0/4.
                || a >= 240)
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_public_address(IpAddr::V4(v4));
            }
            let segments = v6.segments();
            // NAT64 (64:ff9b::/96) y 6to4 (2002::/16) llevan una IPv4 dentro.
            if segments[0] == 0x0064 && segments[1] == 0xff9b {
                let v4 = std::net::Ipv4Addr::new(
                    (segments[6] >> 8) as u8,
                    segments[6] as u8,
                    (segments[7] >> 8) as u8,
                    segments[7] as u8,
                );
                return is_public_address(IpAddr::V4(v4));
            }
            if segments[0] == 0x2002 {
                let v4 = std::net::Ipv4Addr::new(
                    (segments[1] >> 8) as u8,
                    segments[1] as u8,
                    (segments[2] >> 8) as u8,
                    segments[2] as u8,
                );
                return is_public_address(IpAddr::V4(v4));
            }
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                // IPv4 compatibles (::a.b.c.d), obsoletas pero resolubles.
                || (segments[..6].iter().all(|segment| *segment == 0))
                // Direcciones únicas locales (fc00::/7), el equivalente a las privadas de IPv4.
                || (segments[0] & 0xfe00) == 0xfc00
                // Enlace local (fe80::/10).
                || (segments[0] & 0xffc0) == 0xfe80
                // Documentación, 2001:db8::/32.
                || (segments[0] == 0x2001 && segments[1] == 0x0db8))
        }
    }
}

fn reject_private_destinations(addresses: &[SocketAddr]) -> Result<(), AppError> {
    if addresses.is_empty() {
        return Err(AppError::BrokerTransport(
            "el destino no resolvió ninguna dirección".to_owned(),
        ));
    }
    for address in addresses {
        reject_private_address(address.ip())?;
    }
    Ok(())
}

/// Resuelve el destino una sola vez y comprueba cada dirección obtenida.
///
/// Devuelve las direcciones admitidas para que la conexión use exactamente
/// esas: si el cliente HTTP volviera a resolver el nombre, una segunda
/// respuesta DNS distinta —la técnica de «DNS rebinding»— podría llevarlo a la
/// red local después de haber superado la comprobación.
async fn resolve_admitted_addresses(
    url: &Url,
    policy: AddressPolicy,
) -> Result<Vec<SocketAddr>, AppError> {
    let host = url
        .host_str()
        .ok_or_else(|| AppError::Validation("la URL indicada no tiene servidor".to_owned()))?
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_owned();
    let port = url.port_or_known_default().unwrap_or(80);
    let resolver = policy.resolver();
    let addresses = tauri::async_runtime::spawn_blocking(move || resolver(&host, port))
        .await
        .map_err(|error| AppError::BrokerTransport(error.to_string()))?
        .map_err(|error| {
            AppError::BrokerTransport(format!("no se pudo resolver el destino: {error}"))
        })?;
    policy.admit(&addresses)?;
    Ok(addresses)
}

type Resolver = fn(&str, u16) -> std::io::Result<Vec<SocketAddr>>;

fn system_resolver(host: &str, port: u16) -> std::io::Result<Vec<SocketAddr>> {
    (host, port).to_socket_addrs().map(|items| items.collect())
}

/// Qué direcciones puede alcanzar una descarga.
///
/// En producción solo hay una política. La variante de pruebas sustituye el
/// resolvedor y admite el bucle local, que es donde vive el servidor de la
/// prueba; así se puede demostrar que la conexión va a la dirección validada
/// y no a la que devolvería una segunda consulta DNS.
#[derive(Clone, Copy)]
pub(crate) enum AddressPolicy {
    PublicOnly,
    #[cfg(test)]
    Test {
        resolver: Resolver,
        allow_loopback: bool,
    },
}

impl AddressPolicy {
    fn resolver(self) -> Resolver {
        match self {
            Self::PublicOnly => system_resolver,
            #[cfg(test)]
            Self::Test { resolver, .. } => resolver,
        }
    }

    fn admit(self, addresses: &[SocketAddr]) -> Result<(), AppError> {
        match self {
            Self::PublicOnly => reject_private_destinations(addresses),
            #[cfg(test)]
            Self::Test { allow_loopback, .. } => {
                if allow_loopback && addresses.iter().all(|address| address.ip().is_loopback()) {
                    Ok(())
                } else {
                    reject_private_destinations(addresses)
                }
            }
        }
    }
}

/// Cliente de un solo salto, clavado a las direcciones ya validadas.
///
/// Sin proxies automáticos: una variable `HTTP_PROXY` del sistema desviaría la
/// conexión a otro equipo, que resolvería el nombre por su cuenta y dejaría
/// sin efecto la comprobación.
fn pinned_client(url: &Url, addresses: &[SocketAddr]) -> Result<Client, AppError> {
    let mut builder = Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(FETCH_TIMEOUT_SECONDS))
        // Cada salto se sigue manualmente en `fetch_url`: así se valida su
        // destino literal y su resolución DNS antes de abrir la conexión.
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .user_agent(concat!("ChatyGPT/", env!("CARGO_PKG_VERSION")));
    if let Some(Host::Domain(domain)) = url.host() {
        builder = builder.resolve_to_addrs(domain, addresses);
    }
    builder
        .build()
        .map_err(|error| AppError::BrokerTransport(error.to_string()))
}

#[cfg(test)]
fn redirect_target(current: &Url, location: &str) -> Result<Url, AppError> {
    redirect_target_with_policy(current, location, AddressPolicy::PublicOnly)
}

fn redirect_target_with_policy(
    current: &Url,
    location: &str,
    policy: AddressPolicy,
) -> Result<Url, AppError> {
    let target = current.join(location).map_err(|_| {
        AppError::Validation("la redirección contiene una URL no válida".to_owned())
    })?;
    validate_fetch_url_with_policy(target.as_str(), policy)
}

/// Igual que `validate_fetch_url`, pero la variante de pruebas admite el
/// bucle local donde escucha su servidor.
fn validate_fetch_url_with_policy(raw: &str, policy: AddressPolicy) -> Result<Url, AppError> {
    match policy {
        AddressPolicy::PublicOnly => validate_fetch_url(raw),
        #[cfg(test)]
        AddressPolicy::Test {
            allow_loopback: true,
            ..
        } => {
            let url = Url::parse(raw.trim())
                .map_err(|_| AppError::Validation("la URL indicada no es válida".to_owned()))?;
            if url.host_str() == Some("127.0.0.1")
                || url.host_str().is_some_and(|host| host.ends_with(".test"))
            {
                Ok(url)
            } else {
                validate_fetch_url(raw)
            }
        }
        #[cfg(test)]
        AddressPolicy::Test { .. } => validate_fetch_url(raw),
    }
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
    let start = find_ascii_case_insensitive(html, 0, "<title")?;
    let open_end = html[start..].find('>')? + start + 1;
    let end = find_ascii_case_insensitive(html, open_end, "</title>")?;
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
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut result = String::with_capacity(html.len());
    let mut cursor = 0;
    while let Some(start) = find_ascii_case_insensitive(html, cursor, &open) {
        result.push_str(&html[cursor..start]);
        match find_ascii_case_insensitive(html, start, &close) {
            Some(end) => cursor = end + close.len(),
            None => return result,
        }
    }
    result.push_str(&html[cursor..]);
    result
}

/// Busca sintaxis HTML ASCII sin transformar el texto. Convertir todo el HTML
/// a minúsculas puede cambiar su longitud en bytes (por ejemplo, `İ`) y vuelve
/// inválidos los índices al aplicarlos después sobre la cadena original.
fn find_ascii_case_insensitive(haystack: &str, from: usize, needle: &str) -> Option<usize> {
    haystack
        .as_bytes()
        .get(from..)?
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
        .map(|offset| from + offset)
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

/// Ejecuta una consulta GET sin credenciales, cuerpo ni cabeceras aportadas por
/// el modelo. Las redirecciones se desactivan para que una URL pública no pueda
/// saltar después a la red local.
pub fn external_api_get(raw_url: &str) -> Result<ExternalApiResponse, AppError> {
    external_api_get_with_auth(raw_url, None)
}

/// Variante autenticada. El secreto solo se convierte en cabecera dentro de
/// este cliente y nunca forma parte de la URL, la respuesta ni los registros.
pub fn external_api_get_with_auth(
    raw_url: &str,
    authentication: Option<(&str, &str)>,
) -> Result<ExternalApiResponse, AppError> {
    use std::io::Read;

    let url = validate_external_api_url(raw_url)?;
    let host = url
        .host_str()
        .ok_or_else(|| AppError::Validation("la API no tiene servidor".to_owned()))?;
    let port = url.port_or_known_default().unwrap_or(443);
    let addresses = (host.trim_start_matches('[').trim_end_matches(']'), port)
        .to_socket_addrs()
        .map_err(|error| {
            AppError::BrokerTransport(format!("no se pudo resolver el destino de la API: {error}"))
        })?
        .collect::<Vec<_>>();
    reject_private_destinations(&addresses)?;
    // La conexión usa exactamente las direcciones comprobadas y nunca un
    // proxy del sistema: ni una segunda resolución ni un intermediario pueden
    // llevar la credencial de la API a la red local.
    let mut builder = BlockingClient::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(FETCH_TIMEOUT_SECONDS))
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .user_agent(concat!("ChatyGPT/", env!("CARGO_PKG_VERSION")));
    if let Some(Host::Domain(domain)) = url.host() {
        builder = builder.resolve_to_addrs(domain, &addresses);
    }
    let client = builder
        .build()
        .map_err(|error| AppError::BrokerTransport(error.to_string()))?;
    let mut request = client.get(url).header(
        reqwest::header::ACCEPT,
        "application/json, application/problem+json, text/plain, application/xml, text/xml",
    );
    if let Some((mode, secret)) = authentication {
        let (name, value) = match mode {
            "bearer" => (reqwest::header::AUTHORIZATION, format!("Bearer {secret}")),
            "api_key" => (
                reqwest::header::HeaderName::from_static("x-api-key"),
                secret.to_owned(),
            ),
            _ => {
                return Err(AppError::Validation(
                    "el tipo de autenticación API no es válido".to_owned(),
                ))
            }
        };
        let mut value = reqwest::header::HeaderValue::from_str(&value).map_err(|_| {
            AppError::Validation(
                "la credencial contiene caracteres no admitidos en una cabecera HTTP".to_owned(),
            )
        })?;
        value.set_sensitive(true);
        request = request.header(name, value);
    }
    let response = request
        .send()
        .map_err(|error| AppError::BrokerTransport(error.to_string()))?;
    let status = response.status();
    if status.is_redirection() {
        return Err(AppError::Validation(
            "la API intentó redirigir la llamada; usa directamente su URL HTTPS final".to_owned(),
        ));
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    if let Some(media_type) = content_type.as_deref() {
        let media_type = media_type.split(';').next().unwrap_or_default().trim();
        let textual = media_type.starts_with("text/")
            || media_type == "application/json"
            || media_type.ends_with("+json")
            || media_type == "application/xml"
            || media_type.ends_with("+xml");
        if !textual {
            return Err(AppError::Validation(format!(
                "la API devolvió un contenido no textual ({media_type})"
            )));
        }
    }
    let mut bytes = Vec::new();
    response
        .take((MAX_FETCH_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| AppError::BrokerTransport(error.to_string()))?;
    let bytes_truncated = bytes.len() > MAX_FETCH_BYTES;
    if bytes_truncated {
        bytes.truncate(MAX_FETCH_BYTES);
    }
    let decoded = String::from_utf8_lossy(&bytes);
    let character_truncated = decoded.chars().count() > MAX_FETCH_CHARACTERS;
    let body = decoded.chars().take(MAX_FETCH_CHARACTERS).collect();
    Ok(ExternalApiResponse {
        url: raw_url.trim().to_owned(),
        status: status.as_u16(),
        content_type,
        body,
        truncated: bytes_truncated || character_truncated,
    })
}

/// Abre una página y devuelve su texto.
pub async fn fetch_url(raw_url: &str) -> Result<FetchedPage, AppError> {
    fetch_url_with_policy(raw_url, AddressPolicy::PublicOnly).await
}

pub(crate) async fn fetch_url_with_policy(
    raw_url: &str,
    policy: AddressPolicy,
) -> Result<FetchedPage, AppError> {
    let mut url = validate_fetch_url_with_policy(raw_url, policy)?;
    let mut followed_redirects = 0;
    let mut response = loop {
        // Cada salto se resuelve y valida una vez, y su conexión queda clavada
        // a esas direcciones.
        let addresses = resolve_admitted_addresses(&url, policy).await?;
        let client = pinned_client(&url, &addresses)?;
        let response = client
            .get(url.clone())
            .send()
            .await
            .map_err(|error| AppError::BrokerTransport(error.to_string()))?;
        if !response.status().is_redirection() {
            break response;
        }
        if followed_redirects >= MAX_REDIRECTS {
            return Err(AppError::Validation(format!(
                "la página supera el máximo de {MAX_REDIRECTS} redirecciones"
            )));
        }
        let location = response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| {
                AppError::Validation("la redirección no indica un destino válido".to_owned())
            })?;
        url = redirect_target_with_policy(&url, location, policy)?;
        followed_redirects += 1;
    };
    let final_url = url.to_string();
    let status = response.status();
    if !status.is_success() {
        return Err(AppError::BrokerResponse {
            status: status.as_u16(),
            message: format!("la página respondió HTTP {}", status.as_u16()),
        });
    }
    let mut bytes = Vec::with_capacity(MAX_FETCH_BYTES.min(64 * 1024));
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| AppError::BrokerTransport(error.to_string()))?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_FETCH_BYTES {
            return Err(AppError::Validation(format!(
                "la página supera el límite local de {} MB",
                MAX_FETCH_BYTES / (1024 * 1024)
            )));
        }
        bytes.extend_from_slice(&chunk);
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
    use super::{
        extract_readable_text, extract_title, redirect_target, reject_private_destinations,
        validate_external_api_url, validate_fetch_url, MAX_FETCH_CHARACTERS,
    };
    use crate::error::AppError;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use url::Url;

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
    fn external_apis_require_https_and_public_destinations() {
        assert!(validate_external_api_url("https://api.example.org/v1/weather?q=Arrecife").is_ok());
        assert!(validate_external_api_url("http://api.example.org/v1/weather").is_err());
        assert!(validate_external_api_url("https://127.0.0.1:8765/api/v1/tasks").is_err());
        assert!(validate_external_api_url("https://user:secret@example.org/v1").is_err());
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
            "http://[::ffff:127.0.0.1]/",
            "http://[::ffff:192.168.1.52]/",
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
    fn dns_results_and_redirect_targets_cannot_reach_private_networks() {
        let loopback = [SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 80)];
        assert!(reject_private_destinations(&loopback).is_err());

        let public = Url::parse("https://example.org/article").unwrap();
        assert!(redirect_target(&public, "/next").is_ok());
        assert!(redirect_target(&public, "http://127.0.0.1/private").is_err());
        assert!(redirect_target(&public, "http://[::ffff:127.0.0.1]/private").is_err());
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

    #[test]
    fn unicode_before_html_tags_never_invalidates_byte_boundaries() {
        assert_eq!(
            extract_title("İ<TITLE>éxito</TITLE>").as_deref(),
            Some("éxito")
        );
        let (text, _) = extract_readable_text("İ<script>oculto</script>é", 100);
        assert_eq!(text, "İé");
        assert!(!text.contains("oculto"));
    }

    mod dns_fijado {
        //! H03 (auditoría 28-sep-2026): la conexión va a la dirección validada.
        use super::super::{fetch_url_with_policy, is_public_address, AddressPolicy};
        use std::io::{Read, Write};
        use std::net::{IpAddr, SocketAddr, TcpListener};
        use std::sync::atomic::{AtomicU16, AtomicUsize, Ordering};
        use std::sync::Arc;

        static PORT: AtomicU16 = AtomicU16::new(0);

        fn resolves_to_loopback(_host: &str, _port: u16) -> std::io::Result<Vec<SocketAddr>> {
            Ok(vec![SocketAddr::from((
                [127, 0, 0, 1],
                PORT.load(Ordering::SeqCst),
            ))])
        }

        /// Servidor de una página que cuenta las conexiones que recibe.
        fn page_server() -> (u16, Arc<AtomicUsize>) {
            let listener = TcpListener::bind("127.0.0.1:0").expect("puerto libre");
            let port = listener.local_addr().expect("dirección").port();
            let connections = Arc::new(AtomicUsize::new(0));
            let counter = connections.clone();
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let Ok(mut stream) = stream else { continue };
                    counter.fetch_add(1, Ordering::SeqCst);
                    let mut buffer = [0_u8; 2048];
                    let _ = stream.read(&mut buffer);
                    let body = "<title>Fijada</title><p>contenido servido</p>";
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                }
            });
            (port, connections)
        }

        #[test]
        fn the_connection_uses_the_addresses_that_passed_validation() {
            let (port, connections) = page_server();
            PORT.store(port, Ordering::SeqCst);
            // «rebind.test» no existe en ningún DNS: si el cliente volviera a
            // resolver el nombre, fallaría. Solo llega si usa lo validado.
            let page = tauri::async_runtime::block_on(fetch_url_with_policy(
                &format!("http://rebind.test:{port}/pagina"),
                AddressPolicy::Test {
                    resolver: resolves_to_loopback,
                    allow_loopback: true,
                },
            ))
            .expect("la conexión debe ir a la dirección fijada");
            assert!(page.text.contains("contenido servido"));
            assert_eq!(connections.load(Ordering::SeqCst), 1);
        }

        #[test]
        fn a_name_that_resolves_to_the_local_network_never_receives_a_connection() {
            let (port, connections) = page_server();
            PORT.store(port, Ordering::SeqCst);
            let error = tauri::async_runtime::block_on(fetch_url_with_policy(
                &format!("http://rebind.test:{port}/pagina"),
                AddressPolicy::Test {
                    resolver: resolves_to_loopback,
                    allow_loopback: false,
                },
            ))
            .expect_err("una resolución privada debe rechazarse");
            assert!(error.to_string().contains("red local"), "{error}");
            std::thread::sleep(std::time::Duration::from_millis(100));
            assert_eq!(connections.load(Ordering::SeqCst), 0);
        }

        #[test]
        fn special_purpose_ranges_are_not_public() {
            for address in [
                "100.64.0.1",
                "100.127.255.254",
                "198.18.0.1",
                "192.0.0.8",
                "192.0.2.1",
                "224.0.0.1",
                "240.0.0.1",
                "255.255.255.255",
                "0.1.2.3",
                "64:ff9b::7f00:1",
                "64:ff9b::c0a8:0134",
                "2002:c0a8:0134::1",
                "::127.0.0.1",
                "2001:db8::1",
                "ff02::1",
            ] {
                let ip: IpAddr = address.parse().expect("dirección válida");
                assert!(!is_public_address(ip), "debía rechazarse: {address}");
            }
            for address in ["93.184.216.34", "2606:4700:4700::1111", "100.128.0.1"] {
                let ip: IpAddr = address.parse().expect("dirección válida");
                assert!(is_public_address(ip), "debía admitirse: {address}");
            }
        }
    }
}
