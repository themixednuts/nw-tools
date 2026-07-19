//! Legacy LmbrCentral ObjectStream component readers.

mod attachment;
mod particle;
mod read;

pub use attachment::{AttachmentComponentSource, read_attachment_component};
pub use particle::{ParticleComponentSource, read_particle_component};
pub use read::LmbrCentralObjectStreamError;

#[cfg(test)]
mod attachment_tests;
#[cfg(test)]
mod particle_tests;
