//! C ABI stub for plugin / host integration.

use std::ptr;

use crate::engine::SynthEngine;
use crate::patch::Patch;
use crate::wavetable::WavetableBank;

/// Opaque handle wrapping [`SynthEngine`].
pub struct ReelsynthHandle {
    engine: SynthEngine,
}

/// Create a synth instance. Returns null on failure.
///
/// # Safety
/// `bank_path` must be a valid null-terminated UTF-8 path when non-null.
#[no_mangle]
pub unsafe extern "C" fn reelsynth_create(
    bank_path: *const std::os::raw::c_char,
    sample_rate: u32,
) -> *mut ReelsynthHandle {
    if bank_path.is_null() {
        return ptr::null_mut();
    }
    let path = match std::ffi::CStr::from_ptr(bank_path).to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    let bank = match WavetableBank::read_file(path) {
        Ok(b) => b,
        Err(_) => return ptr::null_mut(),
    };
    let engine = SynthEngine::new(bank, Patch::default_mono(), sample_rate);
    Box::into_raw(Box::new(ReelsynthHandle { engine }))
}

/// Render `frames` mono samples into `out`.
///
/// # Safety
/// `handle` must be a valid pointer from `reelsynth_create`. `out` must hold at least `frames` elements.
#[no_mangle]
pub unsafe extern "C" fn reelsynth_process(
    handle: *mut ReelsynthHandle,
    out: *mut f32,
    frames: usize,
) {
    if handle.is_null() || out.is_null() || frames == 0 {
        return;
    }
    let slice = std::slice::from_raw_parts_mut(out, frames);
    (*handle).engine.process(slice);
}

/// Trigger note on (velocity 0..127).
///
/// # Safety
/// `handle` must be valid.
#[no_mangle]
pub unsafe extern "C" fn reelsynth_note_on(
    handle: *mut ReelsynthHandle,
    note: u8,
    velocity: u8,
) {
    if handle.is_null() {
        return;
    }
    (*handle)
        .engine
        .note_on(note, velocity as f32 / 127.0);
}

/// Trigger note off.
///
/// # Safety
/// `handle` must be valid.
#[no_mangle]
pub unsafe extern "C" fn reelsynth_note_off(handle: *mut ReelsynthHandle, note: u8) {
    if handle.is_null() {
        return;
    }
    (*handle).engine.note_off(note);
}

/// Destroy a synth instance created by `reelsynth_create`.
///
/// # Safety
/// `handle` must be valid and not used after this call.
#[no_mangle]
pub unsafe extern "C" fn reelsynth_destroy(handle: *mut ReelsynthHandle) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}
