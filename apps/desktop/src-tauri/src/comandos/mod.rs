//! Las ordenes que la interfaz puede invocar, agrupadas por dominio.
//!
//! Estaban las 143 en `lib.rs`, que llego a 3.200 lineas. Se reexportan sin
//! calificar para que la lista de `generate_handler!` siga leyendose de un
//! vistazo y para que el contrato con la interfaz no cambie.

mod arranque;
mod athena_area;
mod chat;
mod conocimiento;
mod conversaciones;
mod ficheros;
mod gpts;
mod permisos;
mod programacion;
mod tareas;
mod workflows;

pub(crate) use arranque::*;
pub(crate) use athena_area::*;
pub(crate) use chat::*;
pub(crate) use conocimiento::*;
pub(crate) use conversaciones::*;
pub(crate) use ficheros::*;
pub(crate) use gpts::*;
pub(crate) use permisos::*;
pub(crate) use programacion::*;
pub(crate) use tareas::*;
pub(crate) use workflows::*;
