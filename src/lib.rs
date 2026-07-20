pub mod wavetable;
pub mod performance;
pub mod patch;
pub mod osc;
pub mod fm;
pub mod fx;
pub mod overtone;
pub mod crackle_diag;
pub mod crackle_eam;
pub mod artifact_reduce;
pub mod denoise_opt;
pub mod denoise_meta;
pub mod denoise_meta_overnight;
pub mod sound_bench;
pub mod seam;
pub mod signal_library;
pub mod voice;
pub mod scope;
pub mod import;
pub mod export;
pub mod engine;
pub mod ffi;
pub mod lfo;
pub mod wt_quant;
pub mod modulation;
pub mod oversample;
pub mod sequence;
pub mod analysis;

pub use performance::{
    build_pool, note_in_scale, resolve_chord, resolve_diatonic_chord, scale_degree_to_midi,
    snap_note, ArpDirection, ArpEngine, ArpEvent, ArpInputMode, ArpRate, ArpSettings, ArpStep,
    ChordQuality, ChordSet, ChordVoicing, PerformanceLayout, PerformanceSettings, Scale,
    ScaleBehavior,
};
pub use wavetable::WavetableBank;
pub use fx::{default_effects, effects_from_bypass, EffectSlot, EffectType, FxBypass, FxChain};
pub use overtone::{
    curve_harshness, OvertoneFilterChain, OvertoneFilterSlot, OvertoneFilterType,
};
pub use seam::{periodize_cycle, seam_mode_to_crackle, CrackleVoice, SeamStyle};
pub use patch::{
    filter_type_label, legacy_filter_slots, normalize_filter_type, Envelope, Filter, FilterSlot,
    FILTER_TYPES, Macro, ModSlot, Oscillator, Patch, WaveLayer, WaveSlot,
};
pub use wt_quant::{generate_even_wave_slots, resolve_wt_position, resolved_wave_slots};
pub use voice::{render_note, render_note_single_bank};
pub use scope::{
    render_combined_osc_cycle, render_osc_cycle_at_index, render_scope_previews,
    spectrum_magnitudes, ScopePreviews, ScopeTap, PREVIEW_FIFTH_NOTE, PREVIEW_ROOT_NOTE,
};
pub use scope::{ScopeLiveTaps, ScopeMonitor, ScopeRingBuffer, SCOPE_DISPLAY_LEN, SCOPE_RING_LEN};
pub use engine::{BankSet, MidiEvent, SynthEngine, VoiceMpe};
pub use lfo::{lfo_value, LfoRuntime};
pub use modulation::{apply_mods_to_patch, compute_macro_mods, compute_mods, merge_mods, ModSources};
pub use sequence::{
    AutomationLane, AutomationPoint, Clip, ClipRef, MidiNote, QuantizeDivision, QuantizeGrid,
    Scene, SequenceProject, Track, TransportState,
};
pub use export::{
    export_preset, export_reelpack, export_wavetable, load_preset, parse_targets,
    resolve_bank_for_preset, ExportOptions, ExportReport, ExportTarget,
};
pub use analysis::{decompose_frame, resynthesis_error, resynthesize_frame};

#[cfg(feature = "python")]
use numpy::{PyArray1, PyReadonlyArray1};
#[cfg(feature = "python")]
use pyo3::prelude::*;

#[cfg(feature = "python")]
#[pyfunction]
fn modulated_one_pole_lowpass<'py>(
    py: Python<'py>,
    wave: PyReadonlyArray1<f32>,
    base_cutoff: f32,
    cutoff_mod: PyReadonlyArray1<f32>,
    sample_rate: i32,
) -> Bound<'py, PyArray1<f32>> {
    let w = wave.as_slice().unwrap();
    let m = cutoff_mod.as_slice().unwrap();
    let n = w.len().min(m.len());
    let sr = sample_rate as f32;
    let mut out = vec![0.0f32; n];
    let mut y = 0.0f32;
    for i in 0..n {
        let cutoff = (base_cutoff + m[i]).max(25.0);
        let alpha = cutoff / (cutoff + sr * 0.55);
        y += alpha * (w[i] - y);
        out[i] = y;
    }
    PyArray1::from_vec_bound(py, out)
}

#[cfg(feature = "python")]
#[pyfunction]
fn render_note_py<'py>(
    py: Python<'py>,
    bank_path: &str,
    freq: f32,
    duration: f32,
    patch_json: &str,
    sample_rate: u32,
) -> PyResult<Bound<'py, PyArray1<f32>>> {
    let bank = WavetableBank::read_file(bank_path)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e))?;
    let patch = Patch::from_json(patch_json)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e))?;
    let audio = render_note_single_bank(&bank, freq, duration, sample_rate, &patch);
    Ok(PyArray1::from_vec_bound(py, audio))
}

#[cfg(feature = "python")]
#[pyfunction]
fn write_factory_wavetables(output_dir: &str) -> PyResult<Vec<String>> {
    std::fs::create_dir_all(output_dir)
        .map_err(|e| pyo3::exceptions::PyOSError::new_err(e.to_string()))?;
    let factories: Vec<(&str, WavetableBank)> = vec![
        ("saw_morph", WavetableBank::factory_saw_morph()),
        ("square_morph", WavetableBank::factory_square_morph()),
        ("sine", WavetableBank::factory_sine()),
        ("formant", WavetableBank::factory_formant()),
        ("metallic", WavetableBank::factory_metallic()),
    ];
    let mut paths = Vec::new();
    for (name, bank) in factories {
        let path = format!("{output_dir}/{name}.reelwt");
        bank.write_file(&path)
            .map_err(|e| pyo3::exceptions::PyOSError::new_err(e))?;
        paths.push(path);
    }
    Ok(paths)
}

#[cfg(feature = "python")]
#[pyfunction]
fn import_wavetable(source: &str, input_path: &str, output_path: &str) -> PyResult<String> {
    import::import_to_reelwt(source, input_path, output_path)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e))?;
    Ok(output_path.to_string())
}

#[cfg(feature = "python")]
#[pyfunction]
fn bank_info(path: &str) -> PyResult<(usize, usize)> {
    let bank = WavetableBank::read_file(path)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e))?;
    Ok((bank.num_frames, bank.frame_size))
}

#[cfg(feature = "python")]
fn report_to_py(py: Python<'_>, report: export::ExportReport) -> PyResult<PyObject> {
    let json = serde_json::to_string(&report)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    let mod_json = py.import_bound("json")?;
    let obj = mod_json.call_method1("loads", (json,))?;
    Ok(obj.unbind())
}

#[cfg(feature = "python")]
#[pyfunction]
fn export_wavetable_py(
    py: Python<'_>,
    format: &str,
    input_path: &str,
    output_path: &str,
    table_name: Option<&str>,
) -> PyResult<PyObject> {
    let target = export::ExportTarget::parse(format)
        .ok_or_else(|| pyo3::exceptions::PyValueError::new_err(format!("unknown target: {format}")))?;
    let bank = WavetableBank::read_file(input_path)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e))?;
    let opts = export::ExportOptions {
        table_name: table_name.unwrap_or("reelsynth").into(),
        ..export::ExportOptions::default()
    };
    let report = export::export_wavetable(&bank, target, std::path::Path::new(output_path), &opts);
    report_to_py(py, report)
}

#[cfg(feature = "python")]
#[pyfunction]
fn export_preset_py(
    py: Python<'_>,
    format: &str,
    preset_path: &str,
    output_path: &str,
    bank_path: Option<&str>,
) -> PyResult<PyObject> {
    let target = export::ExportTarget::parse(format)
        .ok_or_else(|| pyo3::exceptions::PyValueError::new_err(format!("unknown target: {format}")))?;
    let preset = export::load_preset(std::path::Path::new(preset_path))
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e))?;
    let bank = if let Some(bp) = bank_path {
        WavetableBank::read_file(bp)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e))?
    } else {
        export::resolve_bank_for_preset(std::path::Path::new(preset_path), &preset)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e))?
    };
    let report = export::export_preset(
        &preset,
        &bank,
        target,
        std::path::Path::new(output_path),
        &export::ExportOptions::default(),
    );
    report_to_py(py, report)
}

#[cfg(feature = "python")]
#[pyfunction]
fn export_reelpack_py(
    py: Python<'_>,
    preset_path: &str, out_dir: &str, targets_json: Option<&str>) -> PyResult<PyObject> {
    let targets: Vec<export::ExportTarget> = if let Some(raw) = targets_json {
        let parsed: Vec<String> = serde_json::from_str(raw)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        parsed
            .iter()
            .filter_map(|t| export::ExportTarget::parse(t))
            .collect()
    } else {
        export::parse_targets("vital,wav,serum,ableton,sfz,midi,audio")
    };
    let report = export::export_reelpack(
        std::path::Path::new(preset_path),
        std::path::Path::new(out_dir),
        &targets,
        &export::ExportOptions::default(),
    );
    report_to_py(py, report)
}

#[cfg(feature = "python")]
#[pymodule]
fn reelsynth(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(modulated_one_pole_lowpass, m)?)?;
    m.add_function(wrap_pyfunction!(render_note_py, m)?)?;
    m.add_function(wrap_pyfunction!(write_factory_wavetables, m)?)?;
    m.add_function(wrap_pyfunction!(import_wavetable, m)?)?;
    m.add_function(wrap_pyfunction!(bank_info, m)?)?;
    m.add_function(wrap_pyfunction!(export_wavetable_py, m)?)?;
    m.add_function(wrap_pyfunction!(export_preset_py, m)?)?;
    m.add_function(wrap_pyfunction!(export_reelpack_py, m)?)?;
    m.add("DEFAULT_NUM_FRAMES", wavetable::DEFAULT_NUM_FRAMES)?;
    m.add("DEFAULT_FRAME_SIZE", wavetable::DEFAULT_FRAME_SIZE)?;
    Ok(())
}

// Also expose grok_dsp alias for backward compat during migration
#[cfg(feature = "python")]
#[pymodule]
fn grok_dsp(m: &Bound<'_, PyModule>) -> PyResult<()> {
    reelsynth(m)
}
