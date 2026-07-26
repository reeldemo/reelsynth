//! Integration Q&A — FFI, export fixtures, smoke tests.

use std::ffi::CString;
use std::path::PathBuf;

use reelsynth::export::{
    export_ableton_map, export_ableton_multicycle_wav, export_midi, export_reelpack, export_serum_wt,
    export_sfz, ExportOptions, ExportTarget,
};
use reelsynth::ffi::{reelsynth_create, reelsynth_destroy, reelsynth_note_on, reelsynth_process};
use reelsynth::patch::Patch;
use reelsynth::wavetable::WavetableBank;

use super::helpers::*;

#[test]
fn ffi_create_note_process_nonzero() {
    let dir = std::env::temp_dir().join("reelsynth_qa_ffi");
    let _ = std::fs::create_dir_all(&dir);
    let bank_path = dir.join("bank.reelwt");
    WavetableBank::factory_sine()
        .write_file(bank_path.to_str().unwrap())
        .expect("write bank");
    let c_path = CString::new(bank_path.to_str().unwrap()).unwrap();
    unsafe {
        let handle = reelsynth_create(c_path.as_ptr(), QA_SR);
        assert!(!handle.is_null());
        reelsynth_note_on(handle, 60, 100);
        let mut out = vec![0.0f32; 2048];
        reelsynth_process(handle, out.as_mut_ptr(), out.len());
        assert!(peak(&out) > 0.01);
        reelsynth_destroy(handle);
    }
}

#[test]
fn export_serum_fixture_report_shape() {
    let bank = WavetableBank::factory_sine();
    let mut preset = Patch::default_mono();
    for i in 0..16 {
        preset.mod_matrix.push(reelsynth::patch::ModSlot {
            source: "lfo1".into(),
            target: format!("macro{}", i + 1),
            amount: 0.1,
            enabled: true,
        });
    }
    let dir = std::env::temp_dir().join("reelsynth_qa_serum");
    let _ = std::fs::create_dir_all(&dir);
    let out = dir.join("test.fxp");
    let report = export_serum_wt(&bank, &preset, &out, "qa");
    assert!(report.success);
    assert!(out.exists());
    let golden_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/export/export_report_serum_golden.json");
    let golden: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&golden_path).unwrap()).unwrap();
    let min_dropped = golden["dropped_count_min"].as_u64().unwrap() as usize;
    assert!(
        report.dropped.len() >= min_dropped,
        "dropped {} < min {}",
        report.dropped.len(),
        min_dropped
    );
}

#[test]
fn export_ableton_map_valid_json() {
    let preset = Patch::factory_wt_lead();
    let dir = std::env::temp_dir().join("reelsynth_qa_ableton");
    let _ = std::fs::create_dir_all(&dir);
    let out = dir.join("map.json");
    let report = export_ableton_map(&preset, &out);
    assert!(report.success);
    let parsed: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(parsed["schema"], "reelsynth-ableton-wt-v2");
    assert!(parsed["parameters"]["filter_freq"].is_number());
    assert!(parsed["live_param_aliases"]["filter_freq"].is_array());
}

#[test]
fn export_ableton_multicycle_in_reelpack() {
    use reelsynth::export::{export_ableton_multicycle_wav, export_reelpack, ExportTarget};
    let dir = std::env::temp_dir().join("reelsynth_qa_ableton_multi");
    let _ = std::fs::remove_dir_all(&dir);
    let src = dir.join("src");
    std::fs::create_dir_all(&src).unwrap();
    let bank = WavetableBank::factory_sine();
    bank.write_file(src.join("demo.reelwt").to_str().unwrap())
        .unwrap();
    let preset = Patch {
        name: "demo".into(),
        wavetable_id: Some("demo".into()),
        ..Patch::default_mono()
    };
    let preset_path = src.join("demo.reelpreset");
    std::fs::write(&preset_path, preset.to_json().unwrap()).unwrap();
    let out = dir.join("out.reelpack");
    let report = export_reelpack(
        &preset_path,
        &out,
        &[ExportTarget::Ableton, ExportTarget::Wav],
        &ExportOptions::default(),
    );
    assert!(report.success, "{:?}", report.errors);
    let multi = out.join("synth/ableton/table_multicycle.wav");
    assert!(multi.is_file(), "missing multicycle wav");
    let map: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("synth/ableton/wavetable_map.json")).unwrap())
            .unwrap();
    assert_eq!(map["schema"], "reelsynth-ableton-wt-v2");
    assert_eq!(map["frames"]["frame_count"], bank.num_frames);
    let _ = export_ableton_multicycle_wav; // keep import used if optimized away
}

#[test]
fn export_sfz_and_midi_smoke() {
    let preset = Patch::factory_va_bass();
    let bank = WavetableBank::factory_saw_morph();
    let dir = std::env::temp_dir().join("reelsynth_qa_export");
    let _ = std::fs::create_dir_all(&dir);
    let opts = ExportOptions {
        sample_rate: QA_SR,
        duration: 0.25,
        freq: 110.0,
        midi_note: 45,
        ..ExportOptions::default()
    };
    let sfz_path = dir.join("patch.sfz");
    let sfz_report = export_sfz(&preset, &bank, &sfz_path, &opts);
    assert!(sfz_report.success);
    let sfz_text = std::fs::read_to_string(&sfz_path).unwrap();
    assert!(sfz_text.contains("<region>"));
    let midi_path = dir.join("preview.mid");
    let midi_report = export_midi(&preset, &midi_path, &opts);
    assert!(midi_report.success);
    let midi_bytes = std::fs::read(&midi_path).unwrap();
    assert!(midi_bytes.starts_with(b"MThd"));
}

#[test]
fn export_fixture_preset_loads() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/export/demo.reelpreset");
    let preset = reelsynth::load_preset(&path).expect("load demo preset");
    assert!(!preset.name.is_empty());
}
