//! Carpetas autorizadas: eleccion, alcance y revocacion.
//!
//! Autorizar una carpeta es una decision del usuario y se toma con un
//! dialogo del sistema: la aplicacion no elige rutas por su cuenta.

use crate::*;

/// Autoriza la carpeta que contiene un archivo recién elegido por la persona.
///
/// Elegir un destino en el selector nativo de Windows *es* la concesión: solo
/// desde ahí puede una carpeta pasar a estar autorizada para escritura.
pub(crate) fn authorize_selected_file(
    database: &Database,
    selected_file: &str,
    purpose: &str,
) -> Result<(), AppError> {
    let path = std::path::Path::new(selected_file);
    let folder = path.parent().ok_or_else(|| {
        AppError::Validation("el destino elegido no tiene carpeta contenedora".to_owned())
    })?;
    let display_name = folder.to_string_lossy().into_owned();
    database.authorize_folder(folder, &display_name, purpose)
}

#[tauri::command]
pub(crate) fn list_authorized_folders(
    state: State<'_, AppState>,
) -> Result<Vec<AuthorizedFolderView>, AppError> {
    state.database.list_authorized_folders()
}

#[tauri::command]
pub(crate) fn pick_gpt_read_folder(
    state: State<'_, AppState>,
) -> Result<Option<AuthorizedFolderView>, AppError> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let script = r#"
            Add-Type -AssemblyName System.Windows.Forms
            $dialog = New-Object System.Windows.Forms.FolderBrowserDialog
            $dialog.Description = 'Elige una carpeta que los GPT personales podrán solicitar leer'
            $dialog.ShowNewFolderButton = $false
            if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
                [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
                [Console]::Write($dialog.SelectedPath)
            }
        "#;
        let output = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-STA", "-Command", script])
            .creation_flags(0x0800_0000)
            .output()
            .map_err(|error| {
                AppError::Validation(format!("no se pudo abrir el selector: {error}"))
            })?;
        if !output.status.success() {
            return Err(AppError::Validation(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ));
        }
        let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if path.is_empty() {
            return Ok(None);
        }
        state
            .database
            .authorize_folder_for_read(std::path::Path::new(&path), &path)?;
        Ok(state
            .database
            .list_authorized_folders()?
            .into_iter()
            .find(|folder| folder.path.eq_ignore_ascii_case(&path)))
    }
    #[cfg(not(target_os = "windows"))]
    Err(AppError::Validation(
        "la selección de carpetas todavía solo está disponible en Windows".to_owned(),
    ))
}

#[tauri::command]
pub(crate) fn pick_gpt_modify_folder(
    state: State<'_, AppState>,
) -> Result<Option<AuthorizedFolderView>, AppError> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let script = r#"
            Add-Type -AssemblyName System.Windows.Forms
            $dialog = New-Object System.Windows.Forms.FolderBrowserDialog
            $dialog.Description = 'Elige una carpeta cuyos archivos de texto podrán modificarse tras tu confirmación'
            $dialog.ShowNewFolderButton = $false
            if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
                [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
                [Console]::Write($dialog.SelectedPath)
            }
        "#;
        let output = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-STA", "-Command", script])
            .creation_flags(0x0800_0000)
            .output()
            .map_err(|error| {
                AppError::Validation(format!("no se pudo abrir el selector: {error}"))
            })?;
        if !output.status.success() {
            return Err(AppError::Validation(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ));
        }
        let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if path.is_empty() {
            return Ok(None);
        }
        state
            .database
            .authorize_folder_for_modify(std::path::Path::new(&path), &path)?;
        Ok(state
            .database
            .list_authorized_folders()?
            .into_iter()
            .find(|folder| folder.path.eq_ignore_ascii_case(&path)))
    }
    #[cfg(not(target_os = "windows"))]
    Err(AppError::Validation(
        "la selección de carpetas todavía solo está disponible en Windows".to_owned(),
    ))
}

#[tauri::command]
pub(crate) fn pick_athena_folder(
    state: State<'_, AppState>,
) -> Result<Option<AuthorizedFolderView>, AppError> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let script = r#"
            Add-Type -AssemblyName System.Windows.Forms
            $dialog = New-Object System.Windows.Forms.FolderBrowserDialog
            $dialog.Description = 'Elige la carpeta en la que Athena podrá trabajar con tu autorización'
            $dialog.ShowNewFolderButton = $true
            if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
                [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
                [Console]::Write($dialog.SelectedPath)
            }
        "#;
        let output = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-STA", "-Command", script])
            .creation_flags(0x0800_0000)
            .output()
            .map_err(|error| {
                AppError::Validation(format!("no se pudo abrir el selector: {error}"))
            })?;
        if !output.status.success() {
            return Err(AppError::Validation(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ));
        }
        let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if path.is_empty() {
            return Ok(None);
        }
        state
            .database
            .authorize_folder_for_athena(std::path::Path::new(&path), &path)?;
        Ok(state
            .database
            .list_authorized_folders()?
            .into_iter()
            .find(|folder| folder.path.eq_ignore_ascii_case(&path)))
    }
    #[cfg(not(target_os = "windows"))]
    Err(AppError::Validation(
        "la selección de carpetas todavía solo está disponible en Windows".to_owned(),
    ))
}

#[tauri::command]
pub(crate) fn revoke_authorized_folder(
    folder_id: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<Vec<AuthorizedFolderView>, AppError> {
    if !confirmed {
        return Err(AppError::Validation(
            "revocar una carpeta autorizada requiere confirmación".to_owned(),
        ));
    }
    state.database.revoke_authorized_folder(&folder_id)?;
    state.database.list_authorized_folders()
}
