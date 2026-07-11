pub mod wavetable;
pub mod patch;
pub mod voice;
pub mod import;

pub use wavetable::WavetableBank;
pub use patch::Patch;
pub use voice::render_note;

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
    let audio = render_note(&bank, freq, duration, sample_rate, &patch);
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
#[pymodule]
fn reelsynth(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(modulated_one_pole_lowpass, m)?)?;
    m.add_function(wrap_pyfunction!(render_note_py, m)?)?;
    m.add_function(wrap_pyfunction!(write_factory_wavetables, m)?)?;
    m.add_function(wrap_pyfunction!(import_wavetable, m)?)?;
    m.add_function(wrap_pyfunction!(bank_info, m)?)?;
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
