mod catalog;
mod common;
mod datasheet;
mod datasheet_browser;
mod dds;
mod objectstream;

use anyhow::Result;
use clap::Subcommand;

use catalog::Catalog;
use datasheet::Datasheet;
use dds::Dds;
use objectstream::ObjectStream;

#[derive(Debug, Subcommand)]
pub enum Cmd {
    #[command(about = "Inspect asset catalog files")]
    Catalog(Catalog),
    #[command(about = "Inspect datasheet files")]
    Datasheet(Datasheet),
    #[command(name = "dds", about = "Inspect or convert DDS texture files")]
    Dds(Dds),
    #[command(name = "model", about = "Convert CGF meshes to glTF (.glb/.gltf)")]
    Model(crate::model::Model),
    #[command(name = "objectstream", about = "Inspect ObjectStream files")]
    ObjectStream(ObjectStream),
}

impl Cmd {
    pub fn run(self) -> Result<()> {
        match self {
            Self::Catalog(cmd) => cmd.run(),
            Self::Datasheet(cmd) => cmd.run(),
            Self::Dds(cmd) => cmd.run(),
            Self::Model(cmd) => cmd.run(),
            Self::ObjectStream(cmd) => cmd.run(),
        }
    }
}
