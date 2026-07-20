//! Audio command channel between UI and DSP thread.

use crossbeam_channel::{Receiver, TryRecvError};
use reelsynth::engine::MidiEvent;
use reelsynth::sequence::{ClipRef, TransportState};
use reelsynth::{Envelope, Macro, ModSlot, Patch, SequenceProject, SynthEngine, WavetableBank};
use std::sync::{Arc, RwLock};

#[allow(dead_code)] // many Set* variants are matched for DSP but constructed via SetPatch paths
pub(crate) enum AudioCmd {
    Midi(MidiEvent),
    TransportPlay,
    TransportStop,
    TransportRecord {
        track: Option<usize>,
    },
    LaunchScene {
        slots: Vec<Option<ClipRef>>,
    },
    SetBpm(f32),
    SeekPlayhead(f32),
    SetSequence(SequenceProject),
    SetRecordArm {
        track: usize,
        armed: bool,
    },
    SetWtPosition(f32),
    SetFilterCutoff(f32),
    SetFilterResonance(f32),
    SetFilterType(String),
    SetFilterKeyTracking(f32),
    SetEnvelope(Envelope),
    SetFilterEnvelope(Envelope),
    SetLfo { rate: f32, depth: f32, shape: String },
    SetLfo2 { rate: f32, depth: f32, shape: String },
    SetMacros(Vec<Macro>),
    SetOsc {
        index: usize,
        level: f32,
        detune: f32,
        unison: u32,
        position: f32,
        pan: f32,
        osc_type: String,
        pulse_width: f32,
        morph_a: f32,
        morph_b: f32,
        morph_amount: f32,
        warp_mode: String,
        warp_amount: f32,
        fm_source: String,
        fm_ratio: f32,
        fm_index: f32,
    },
    SetOscFm {
        index: usize,
        fm_source: String,
        fm_ratio: f32,
        fm_index: f32,
    },
    SetFilterDrive(f32),
    SetFilter2 {
        cutoff: f32,
        resonance: f32,
        filter_type: String,
        drive: f32,
    },
    SetUnisonStereoSpread(f32),
    SetSubLevel(f32),
    SetNoiseLevel(f32),
    SetModMatrix(Vec<ModSlot>),
    SetEffects(Vec<reelsynth::EffectSlot>),
    SetOvertoneSlots(Vec<reelsynth::OvertoneFilterSlot>),
    SetPatch(Patch),
    LoadPreset {
        patch: Patch,
        bank: WavetableBank,
    },
    UpdateBank(WavetableBank),
    NoteOnFreq {
        channel: u8,
        note: u8,
        freq: f32,
        velocity: f32,
    },
}


pub(crate) fn drain_commands(
    engine: &mut SynthEngine,
    rx: &Receiver<AudioCmd>,
    bank_shared: &Arc<RwLock<WavetableBank>>,
    _transport_shared: &Arc<RwLock<TransportState>>,
) {
    loop {
        match rx.try_recv() {
            Ok(AudioCmd::TransportPlay) => engine.sequencer_mut().transport_play(),
            Ok(AudioCmd::TransportStop) => {
                let offs = engine.sequencer().stop_note_offs();
                for ev in offs {
                    if let reelsynth::sequence::SchedEvent::NoteOff { channel, note, .. } = ev {
                        engine.note_off(channel, note);
                    }
                }
                engine.sequencer_mut().transport_stop();
                let mut seq = engine.patch().sequence.clone();
                engine.sequencer_mut().stop_record_and_commit(&mut seq);
                engine.patch_mut().sequence = seq;
            }
            Ok(AudioCmd::LaunchScene { slots }) => {
                let offs = engine.sequencer().stop_note_offs();
                for ev in offs {
                    if let reelsynth::sequence::SchedEvent::NoteOff { channel, note, .. } = ev {
                        engine.note_off(channel, note);
                    }
                }
                engine.sequencer_mut().launch_scene(slots);
            }
            Ok(AudioCmd::TransportRecord { track }) => {
                let mut seq = engine.patch().sequence.clone();
                engine
                    .sequencer_mut()
                    .transport_record(&mut seq, track);
                engine.patch_mut().sequence = seq;
            }
            Ok(AudioCmd::SetBpm(bpm)) => {
                engine.sequencer_mut().set_bpm(bpm);
                engine.patch_mut().sequence.bpm = bpm;
            }
            Ok(AudioCmd::SeekPlayhead(beats)) => engine.sequencer_mut().seek_playhead(beats),
            Ok(AudioCmd::SetSequence(sequence)) => {
                engine.patch_mut().sequence = sequence.clone();
                engine.sequencer_mut().sync_from_project(&sequence);
            }
            Ok(AudioCmd::SetRecordArm { track, armed }) => {
                if let Some(t) = engine.patch_mut().sequence.tracks.get_mut(track) {
                    t.arm = armed;
                }
            }
            Ok(AudioCmd::Midi(event)) => engine.handle_event(event),
            Ok(AudioCmd::SetWtPosition(p)) => engine.set_wt_position(p),
            Ok(AudioCmd::SetFilterCutoff(c)) => engine.set_filter_cutoff(c),
            Ok(AudioCmd::SetFilterResonance(r)) => engine.set_filter_resonance(r),
            Ok(AudioCmd::SetFilterType(t)) => engine.set_filter_type(&t),
            Ok(AudioCmd::SetFilterKeyTracking(kt)) => engine.set_filter_key_tracking(kt),
            Ok(AudioCmd::SetEnvelope(e)) => engine.set_envelope(e),
            Ok(AudioCmd::SetFilterEnvelope(e)) => engine.set_filter_envelope(e),
            Ok(AudioCmd::SetLfo { rate, depth, shape }) => {
                engine.set_lfo_rate(rate);
                engine.set_lfo_depth(depth);
                engine.set_lfo_shape(&shape);
            }
            Ok(AudioCmd::SetLfo2 { rate, depth, shape }) => {
                engine.set_lfo2_rate(rate);
                engine.set_lfo2_depth(depth);
                engine.set_lfo2_shape(&shape);
            }
            Ok(AudioCmd::SetMacros(macros)) => engine.set_macros(macros),
            Ok(AudioCmd::SetOsc {
                index,
                level,
                detune,
                unison,
                position,
                pan,
                osc_type,
                pulse_width,
                morph_a,
                morph_b,
                morph_amount,
                warp_mode,
                warp_amount,
                fm_source,
                fm_ratio,
                fm_index,
            }) => {
                engine.set_osc_level(index, level);
                engine.set_osc_detune(index, detune);
                engine.set_osc_unison(index, unison);
                engine.set_osc_position(index, position);
                engine.set_osc_pan(index, pan);
                engine.set_osc_type(index, &osc_type);
                engine.set_osc_pulse_width(index, pulse_width);
                engine.set_osc_morph(index, morph_a, morph_b, morph_amount);
                engine.set_osc_warp(index, &warp_mode, warp_amount);
                engine.set_osc_fm(index, &fm_source, fm_ratio, fm_index);
            }
            Ok(AudioCmd::SetOscFm {
                index,
                fm_source,
                fm_ratio,
                fm_index,
            }) => engine.set_osc_fm(index, &fm_source, fm_ratio, fm_index),
            Ok(AudioCmd::SetFilterDrive(d)) => engine.set_filter_drive(d),
            Ok(AudioCmd::SetFilter2 {
                cutoff,
                resonance,
                filter_type,
                drive,
            }) => {
                engine.set_filter2_cutoff(cutoff);
                engine.set_filter2_resonance(resonance);
                engine.set_filter2_type(&filter_type);
                engine.set_filter2_drive(drive);
            }
            Ok(AudioCmd::SetUnisonStereoSpread(s)) => engine.set_unison_stereo_spread(s),
            Ok(AudioCmd::SetSubLevel(l)) => engine.set_sub_level(l),
            Ok(AudioCmd::SetNoiseLevel(l)) => engine.set_noise_level(l),
            Ok(AudioCmd::SetModMatrix(slots)) => engine.set_mod_matrix(slots),
            Ok(AudioCmd::SetEffects(effects)) => engine.set_effects(effects),
            Ok(AudioCmd::SetOvertoneSlots(slots)) => engine.set_overtone_slots(slots),
            Ok(AudioCmd::SetPatch(patch)) => engine.apply_patch_hot(patch),
            Ok(AudioCmd::LoadPreset { patch, bank }) => {
                engine.load_preset(bank.clone(), patch);
                if let Ok(mut g) = bank_shared.write() {
                    *g = engine.bank().clone();
                }
            }
            Ok(AudioCmd::UpdateBank(bank)) => {
                engine.update_bank(bank.clone());
                if let Ok(mut g) = bank_shared.write() {
                    *g = bank;
                }
            }
            Ok(AudioCmd::NoteOnFreq {
                channel,
                note,
                freq,
                velocity,
            }) => engine.note_on_freq(channel, note, freq, velocity),
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => break,
        }
    }
}

