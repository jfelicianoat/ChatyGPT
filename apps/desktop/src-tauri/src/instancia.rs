//! Una sola instancia por perfil de datos (auditoría 14-oct, INC-15).
//!
//! Dos ventanas sobre la misma base reanudaban dos veces la misma tarea y
//! escribían sin coordinarse. El candado es por directorio de datos, no por
//! aplicación: abrir otro perfil con `CHATYGPT_DATA_DIR` sigue permitido.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;

/// Toma el candado del perfil. `Ok(None)` significa que otra instancia lo tiene.
///
/// El fichero devuelto debe vivir mientras dure el proceso: el sistema libera
/// el candado al cerrarlo, también si la aplicación termina de forma abrupta.
pub fn adquirir(data_dir: &Path) -> std::io::Result<Option<File>> {
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(data_dir.join("chatygpt.lock"))?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(error)) => Err(error),
    }
}

/// Avisa con un cuadro nativo: la ventana de la aplicación aún no existe.
#[cfg(target_os = "windows")]
pub fn avisar_instancia_abierta() {
    #[link(name = "user32")]
    extern "system" {
        fn MessageBoxW(hwnd: isize, text: *const u16, caption: *const u16, kind: u32) -> i32;
    }
    let wide = |text: &str| text.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let text = wide(
        "ChatyGPT ya está abierto con estos datos.\n\n\
         Usa la ventana que ya tienes abierta: dos ventanas sobre los mismos datos \
         reanudarían las mismas tareas a la vez.",
    );
    let caption = wide("ChatyGPT");
    // MB_OK | MB_ICONINFORMATION
    unsafe {
        MessageBoxW(0, text.as_ptr(), caption.as_ptr(), 0x40);
    }
}

#[cfg(not(target_os = "windows"))]
pub fn avisar_instancia_abierta() {
    eprintln!("ChatyGPT ya está abierto con estos datos.");
}

#[cfg(test)]
mod tests {
    use super::adquirir;

    #[test]
    fn a_second_instance_on_the_same_profile_is_refused_until_the_first_closes() {
        let dir = std::env::temp_dir().join(format!(
            "chatygpt-instancia-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&dir).expect("directorio");
        let primera = adquirir(&dir)
            .expect("io")
            .expect("la primera toma el candado");
        assert!(
            adquirir(&dir).expect("io").is_none(),
            "la segunda debe rechazarse"
        );

        let otro = dir.join("otro-perfil");
        std::fs::create_dir_all(&otro).expect("directorio");
        assert!(
            adquirir(&otro).expect("io").is_some(),
            "otro perfil sí puede abrirse"
        );

        drop(primera);
        assert!(adquirir(&dir).expect("io").is_some(), "al cerrar se libera");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
