use cry_chunk::{CafAnimation, CafControllerChunkScan, CafControllerForm, CafControllerScan};

const INTRO_AZURE_BAG: &[u8] =
    include_bytes!("fixtures/objects/climax/models/azure/intro_azure_bag.caf");
const INTRO_AZURE_CANDLE2: &[u8] =
    include_bytes!("fixtures/objects/climax/models/azure/intro_azure_candle2.caf");

#[test]
fn scans_real_new_world_compressed_controller_formats() {
    let bag = compressed_controllers(INTRO_AZURE_BAG);
    let candle2 = compressed_controllers(INTRO_AZURE_CANDLE2);

    assert!(bag.iter().all(|controller| controller.version == 0x0831));
    assert!(
        candle2
            .iter()
            .all(|controller| controller.version == 0x0831)
    );
    assert!(bag.iter().any(|controller| controller.rotation_format == 8));
    assert!(
        candle2
            .iter()
            .any(|controller| controller.rotation_format == 5)
    );
    assert!(
        candle2
            .iter()
            .any(|controller| controller.rotation_format == 8)
    );
    assert!(bag.iter().all(|controller| {
        controller.position_key_count == 0 || controller.position_format == 2
    }));
    assert!(candle2.iter().all(|controller| {
        controller.position_key_count == 0 || controller.position_format == 2
    }));
    assert!(
        bag.iter()
            .any(|controller| controller.rotation_time_format == 2)
    );
    assert!(
        candle2
            .iter()
            .any(|controller| controller.rotation_time_format == 1)
    );
}

#[test]
fn decodes_real_new_world_fixtures_with_valid_tracks() {
    for bytes in [INTRO_AZURE_BAG, INTRO_AZURE_CANDLE2] {
        let caf = CafAnimation::parse(bytes).expect("fixture CAF decodes");

        assert!(caf.sample_rate.is_finite());
        assert!(caf.sample_rate > 0.0);
        assert!(caf.header.total_duration.is_finite());
        assert!(caf.header.total_duration >= 0.0);
        assert!(caf.header.end_sec >= caf.header.start_sec);
        assert!(
            (caf.header.total_duration - (caf.header.end_sec - caf.header.start_sec)).abs()
                <= 0.001
        );
        assert!(!caf.controllers.is_empty());
        assert_tracks_are_valid(&caf);
    }
}

#[test]
fn decodes_real_new_world_fixture_with_trailing_controller_keys() {
    let caf = CafAnimation::parse(INTRO_AZURE_BAG).expect("fixture CAF decodes");
    let max_key_time = caf
        .controllers
        .iter()
        .flat_map(|controller| {
            controller
                .rotations
                .iter()
                .map(|key| key.time)
                .chain(controller.positions.iter().map(|key| key.time))
        })
        .fold(0.0_f32, f32::max);

    assert!(max_key_time > caf.header.total_duration);
    assert_tracks_are_valid(&caf);
}

fn compressed_controllers(bytes: &[u8]) -> Vec<cry_chunk::CafCompressedControllerScan> {
    let scan = CafControllerScan::scan(bytes).expect("fixture CAF scans");
    assert!(!scan.controllers.is_empty());
    assert!(
        scan.controllers
            .iter()
            .all(|controller| controller.form() == CafControllerForm::Compressed0831)
    );
    scan.controllers
        .into_iter()
        .map(|controller| match controller {
            CafControllerChunkScan::Compressed(controller) => controller,
            _ => unreachable!("fixture scan only has compressed 0x0831 controllers"),
        })
        .collect()
}

fn assert_tracks_are_valid(caf: &CafAnimation) {
    for controller in &caf.controllers {
        assert!(
            controller
                .rotations
                .windows(2)
                .all(|pair| pair[0].time < pair[1].time)
        );
        assert!(
            controller
                .positions
                .windows(2)
                .all(|pair| pair[0].time < pair[1].time)
        );

        for key in &controller.rotations {
            assert!(key.time.is_finite());
            let norm = key
                .value
                .iter()
                .map(|component| component * component)
                .sum::<f32>();
            assert!((norm - 1.0).abs() <= 0.001, "quaternion norm {norm}");
        }
        for key in &controller.positions {
            assert!(key.time.is_finite());
            assert!(key.value.iter().all(|component| component.is_finite()));
        }
    }
}
