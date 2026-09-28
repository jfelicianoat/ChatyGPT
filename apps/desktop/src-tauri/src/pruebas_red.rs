//! Utilidades de red para pruebas: servidores que se portan mal a propósito.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Servidor que responde por trozos, sin `Content-Length`, hasta que el
/// cliente cierra o se alcanzan `max_chunks` trozos de 1 MB.
///
/// Devuelve la URL base y el número de trozos que llegó a escribir: si el
/// cliente corta a tiempo, se queda muy por debajo del máximo.
pub fn endless_chunked_server(max_chunks: usize) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("puerto libre");
    let port = listener.local_addr().expect("dirección").port();
    let written = Arc::new(AtomicUsize::new(0));
    let counter = written.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = [0_u8; 4096];
            let _ = stream.read(&mut buffer);
            if stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n",
                )
                .is_err()
            {
                continue;
            }
            let chunk = vec![b' '; 1024 * 1024];
            for _ in 0..max_chunks {
                let header = format!("{:x}\r\n", chunk.len());
                if stream.write_all(header.as_bytes()).is_err()
                    || stream.write_all(&chunk).is_err()
                    || stream.write_all(b"\r\n").is_err()
                {
                    break;
                }
                counter.fetch_add(1, Ordering::SeqCst);
            }
            let _ = stream.write_all(b"0\r\n\r\n");
        }
    });
    (format!("http://127.0.0.1:{port}"), written)
}
