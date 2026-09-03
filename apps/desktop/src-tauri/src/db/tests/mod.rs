//! Pruebas de la capa de persistencia, repartidas por dominio.
//!
//! Estaban en un solo bloque de 5.500 lineas dentro de `db/mod.rs`. Ahora
//! cada uno vive al lado del dominio que ejercita, que es donde se buscan
//! cuando algo falla.

mod adjuntos;
mod auditoria;
mod comunes;
mod conversaciones;
mod documentos;
mod gpts;
mod herramientas;
mod investigacion;
mod memoria;
mod metricas;
mod programacion;
mod proyectos;
mod resumenes;
mod semantica;
mod workflows;
