//! Los tipos de la capa de persistencia.
//!
//! Estaban todos en `db/mod.rs`, que llego a 1.600 lineas solo de
//! definiciones. Se reexportan aqui para que `crate::db::LoQueSea` siga
//! funcionando igual que antes.

mod conversaciones;
mod gpts;
mod memoria;
mod tareas;

pub(crate) use conversaciones::*;
pub(crate) use gpts::*;
pub(crate) use memoria::*;
pub(crate) use tareas::*;
