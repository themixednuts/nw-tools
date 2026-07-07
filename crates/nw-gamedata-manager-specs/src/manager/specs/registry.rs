use super::*;

mod chunk_00;
mod chunk_01;
mod chunk_02;
mod chunk_03;
mod chunk_04;
mod chunk_05;
mod chunk_06;
mod chunk_07;
mod chunk_08;
mod chunk_09;

pub(super) fn validated_native_manager_specs() -> Vec<NativeManagerSpec> {
    let mut managers = Vec::new();
    managers.extend(chunk_00::specs());
    managers.extend(chunk_01::specs());
    managers.extend(chunk_02::specs());
    managers.extend(chunk_03::specs());
    managers.extend(chunk_04::specs());
    managers.extend(chunk_05::specs());
    managers.extend(chunk_06::specs());
    managers.extend(chunk_07::specs());
    managers.extend(chunk_08::specs());
    managers.extend(chunk_09::specs());
    managers.extend(inputs::runtime_manifest_manager_specs());
    managers
}
