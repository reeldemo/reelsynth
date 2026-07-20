use super::process::*;
use crate::engine::VoiceMpe;
use crate::patch::Patch;
use crate::wavetable::WavetableBank;

    fn single_bank_ctx<'a>(
        bank: &'a WavetableBank,
        patch: &'a Patch,
        freq: f32,
        gate: bool,
        velocity: f32,
        time: f32,
        dt: f32,
    ) -> VoiceSampleContext<'a> {
        VoiceSampleContext {
            banks: std::slice::from_ref(bank),
            bank_for_osc: &|_| 0,
            wt_ids: &[],
            patch,
            freq,
            gate,
            velocity,
            time,
            sample_index: 0,
            dt,
            sr: 44100.0,
            modwheel: 0.0,
            mpe: VoiceMpe::default(),
            bend_range_semitones: 48.0,
        }
    }

    #[test]
    fn velocity_scales_amplitude() {
        let bank = WavetableBank::factory_sine();
        let patch = Patch::default_mono();
        let mut low = VoiceState::new(&patch);
        let mut high = VoiceState::new(&patch);
        let dt = 1.0 / 44100.0;
        for i in 0..4410 {
            let t = i as f32 * dt;
            let ctx_low = single_bank_ctx(&bank, &patch, 440.0, true, 0.25, t, dt);
            let ctx_high = single_bank_ctx(&bank, &patch, 440.0, true, 1.0, t, dt);
            let [l_l, _] = process_sample(&mut low, &ctx_low);
            let [l_h, _] = process_sample(&mut high, &ctx_high);
            if i > 2000 {
                assert!(l_h.abs() > l_l.abs());
            }
        }
    }

    #[test]
    fn pan_moves_energy() {
        let bank = WavetableBank::factory_sine();
        let mut patch_left = Patch::default_mono();
        patch_left.oscillators[0].pan = -1.0;
        let mut patch_right = Patch::default_mono();
        patch_right.oscillators[0].pan = 1.0;
        let mut left_voice = VoiceState::new(&patch_left);
        let mut right_voice = VoiceState::new(&patch_right);
        let dt = 1.0 / 44100.0;
        let mut hard_left = 0.0f32;
        let mut soft_left = 0.0f32;
        let mut hard_right = 0.0f32;
        let mut soft_right = 0.0f32;
        for i in 0..4410 {
            let t = i as f32 * dt;
            let ctx_l = single_bank_ctx(&bank, &patch_left, 440.0, true, 1.0, t, dt);
            let ctx_r = single_bank_ctx(&bank, &patch_right, 440.0, true, 1.0, t, dt);
            let [l, r] = process_sample(&mut left_voice, &ctx_l);
            hard_left += l.abs();
            soft_left += r.abs();
            let [l2, r2] = process_sample(&mut right_voice, &ctx_r);
            soft_right += l2.abs();
            hard_right += r2.abs();
        }
        assert!(hard_left > soft_left * 2.0, "hard_left={hard_left} soft_left={soft_left}");
        assert!(hard_right > soft_right * 2.0, "hard_right={hard_right} soft_right={soft_right}");
    }

    #[test]
    fn va_saw_produces_signal() {
        let bank = WavetableBank::factory_sine();
        let mut patch = Patch::factory_va_bass();
        patch.oscillators.truncate(1);
        patch.oscillators[0].level = 1.0;
        let mut voice = VoiceState::new(&patch);
        let dt = 1.0 / 44100.0;
        let mut peak = 0.0f32;
        for i in 0..4410 {
            let t = i as f32 * dt;
            let ctx = single_bank_ctx(&bank, &patch, 55.0, true, 1.0, t, dt);
            let [l, r] = process_sample(&mut voice, &ctx);
            peak = peak.max(l.abs().max(r.abs()));
        }
        assert!(peak > 0.05, "va saw peak={peak}");
    }

    #[test]
    fn dual_filter_stereo_width() {
        let bank = WavetableBank::factory_sine();
        let mut patch = Patch::default_mono();
        patch.filter.cutoff = 400.0;
        patch.filter2.cutoff = 4000.0;
        patch.filter2.filter_type = "highpass".into();
        patch.oscillators[0].unison = 3;
        patch.unison_stereo_spread = 1.0;
        let mut voice = VoiceState::new(&patch);
        let dt = 1.0 / 44100.0;
        let mut diff = 0.0f32;
        for i in 0..4410 {
            let t = i as f32 * dt;
            let ctx = single_bank_ctx(&bank, &patch, 440.0, true, 1.0, t, dt);
            let [l, r] = process_sample(&mut voice, &ctx);
            if l.is_finite() && r.is_finite() {
                diff += (l - r).abs();
            }
        }
        assert!(diff > 5.0, "stereo diff={diff}");
    }

    #[test]
    fn empty_filter_chain_bypasses_svf() {
        use crate::patch::FilterSlot;
        let bank = WavetableBank::factory_sine();
        let mut open = Patch::default_mono();
        open.filter.cutoff = 80.0;
        open.filter.key_tracking = 0.0;
        open.filter_envelope.sustain = 0.0;
        open.filter_envelope.attack = 0.001;
        open.filter_envelope.decay = 0.001;
        open.filters = None; // legacy dual still filters

        let mut bypass = open.clone();
        bypass.filters = Some(Vec::<FilterSlot>::new());

        let mut dark = open.clone();
        dark.filters = Some(vec![FilterSlot {
            filter_type: "lowpass".into(),
            cutoff: 80.0,
            resonance: 0.2,
            key_tracking: 0.0,
            drive: 0.0,
            bypassed: false,
        }]);

        let dt = 1.0 / 44100.0;
        let n = 8000usize;
        let mut rms_bypass = 0.0f32;
        let mut rms_dark = 0.0f32;
        let mut vb = VoiceState::new(&bypass);
        let mut vd = VoiceState::new(&dark);
        for i in 0..n {
            let t = i as f32 * dt;
            let [lb, _] = process_sample(
                &mut vb,
                &single_bank_ctx(&bank, &bypass, 880.0, true, 1.0, t, dt),
            );
            let [ld, _] = process_sample(
                &mut vd,
                &single_bank_ctx(&bank, &dark, 880.0, true, 1.0, t, dt),
            );
            if i > 2000 {
                rms_bypass += lb * lb;
                rms_dark += ld * ld;
            }
        }
        rms_bypass = (rms_bypass / (n - 2000) as f32).sqrt();
        rms_dark = (rms_dark / (n - 2000) as f32).sqrt();
        assert!(
            rms_bypass > rms_dark * 1.5,
            "empty chain should bypass LP: bypass={rms_bypass} dark={rms_dark}"
        );
    }

    #[test]
    fn filter_chain_series_stacks_and_drive_order() {
        use crate::patch::FilterSlot;
        let bank = WavetableBank::factory_saw_morph();
        let lp = |cutoff: f32, drive: f32| FilterSlot {
            filter_type: "lowpass".into(),
            cutoff,
            resonance: 0.15,
            key_tracking: 0.0,
            drive,
            bypassed: false,
        };
        let mut one = Patch::default_mono();
        one.filters = Some(vec![lp(400.0, 0.0)]);
        one.filter_envelope.sustain = 0.0;
        one.filter_envelope.decay = 0.001;
        let mut two = Patch::default_mono();
        two.filters = Some(vec![lp(400.0, 0.0), lp(400.0, 0.0)]);
        two.filter_envelope.sustain = 0.0;
        two.filter_envelope.decay = 0.001;

        let mut drive_first = Patch::default_mono();
        drive_first.filters = Some(vec![
            FilterSlot {
                filter_type: "lowpass".into(),
                cutoff: 12000.0,
                resonance: 0.0,
                key_tracking: 0.0,
                drive: 0.95,
                bypassed: false,
            },
            lp(400.0, 0.0),
        ]);
        drive_first.filter_envelope.sustain = 0.0;
        drive_first.filter_envelope.decay = 0.001;
        let mut drive_second = Patch::default_mono();
        drive_second.filters = Some(vec![
            lp(400.0, 0.0),
            FilterSlot {
                filter_type: "lowpass".into(),
                cutoff: 12000.0,
                resonance: 0.0,
                key_tracking: 0.0,
                drive: 0.95,
                bypassed: false,
            },
        ]);
        drive_second.filter_envelope.sustain = 0.0;
        drive_second.filter_envelope.decay = 0.001;

        let dt = 1.0 / 44100.0;
        let n = 6000usize;
        let render = |patch: &Patch| {
            let mut v = VoiceState::new(patch);
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                let t = i as f32 * dt;
                let [s, _] = process_sample(
                    &mut v,
                    &single_bank_ctx(&bank, patch, 220.0, true, 1.0, t, dt),
                );
                out.push(s);
            }
            out
        };
        let a = render(&one);
        let b = render(&two);
        let max_diff = a
            .iter()
            .zip(b.iter())
            .skip(1500)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0f32, f32::max);
        assert!(
            max_diff > 0.01,
            "two series LPs must differ from one: max_diff={max_diff}"
        );

        let d1 = render(&drive_first);
        let d2 = render(&drive_second);
        let drive_diff = d1
            .iter()
            .zip(d2.iter())
            .skip(1500)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0f32, f32::max);
        assert!(
            drive_diff > 0.01,
            "drive→LP vs LP→drive must differ: max_diff={drive_diff}"
        );
    }

    /// Diagnostic: held sine after soft-start should not have near-full-scale steps.
    #[test]
    fn held_sine_period_step_bounded() {
        let bank = WavetableBank::factory_sine();
        let mut patch = Patch::default_mono();
        patch.filters = Some(vec![]); // bypass musical filters
        patch.effects.clear();
        patch.lfo.depth = 0.0;
        patch.lfo2.depth = 0.0;
        let mut voice = VoiceState::new(&patch);
        let dt = 1.0 / 44100.0;
        let mut prev = 0.0f32;
        let mut max_step = 0.0f32;
        for i in 0..8000 {
            let t = i as f32 * dt;
            let [s, _] = process_sample(
                &mut voice,
                &single_bank_ctx(&bank, &patch, 440.0, true, 1.0, t, dt),
            );
            if i > 2000 {
                max_step = max_step.max((s - prev).abs());
            }
            prev = s;
        }
        assert!(
            max_step < 0.12,
            "clean sine sustain step={max_step} (voice path discontinuity)"
        );
    }

    #[test]
    fn unison_spread_widens_stereo() {
        let bank = WavetableBank::factory_sine();
        let mut narrow = Patch::default_mono();
        narrow.oscillators[0].unison = 4;
        narrow.unison_stereo_spread = 0.0;
        narrow.filter2 = narrow.filter.clone();
        let mut wide = Patch::default_mono();
        wide.oscillators[0].unison = 4;
        wide.unison_stereo_spread = 1.0;
        wide.filter2 = wide.filter.clone();
        let dt = 1.0 / 44100.0;
        let mut narrow_diff = 0.0f32;
        let mut wide_diff = 0.0f32;
        let mut v1 = VoiceState::new(&narrow);
        let mut v2 = VoiceState::new(&wide);
        for i in 0..4410 {
            let t = i as f32 * dt;
            let [l1, r1] = process_sample(
                &mut v1,
                &single_bank_ctx(&bank, &narrow, 440.0, true, 1.0, t, dt),
            );
            let [l2, r2] = process_sample(
                &mut v2,
                &single_bank_ctx(&bank, &wide, 440.0, true, 1.0, t, dt),
            );
            narrow_diff += (l1 - r1).abs();
            wide_diff += (l2 - r2).abs();
        }
        assert!(wide_diff > narrow_diff * 1.2, "narrow={narrow_diff} wide={wide_diff}");
    }

    #[test]
    fn fm_index_changes_output() {
        let bank = WavetableBank::factory_sine();
        let mut wet_patch = Patch::factory_fm_bell();
        wet_patch.mod_matrix.clear();
        wet_patch.lfo.depth = 0.0;
        let mut dry_patch = wet_patch.clone();
        dry_patch.oscillators[0].fm_source = "none".into();
        dry_patch.oscillators[0].fm_index = 0.0;

        let mut dry = VoiceState::new(&dry_patch);
        let mut wet = VoiceState::new(&wet_patch);
        let dt = 1.0 / 44100.0;
        let mut diff = 0.0f32;
        for i in 0..4410 {
            let t = i as f32 * dt;
            let ctx_dry = single_bank_ctx(&bank, &dry_patch, 880.0, true, 1.0, t, dt);
            let ctx_wet = single_bank_ctx(&bank, &wet_patch, 880.0, true, 1.0, t, dt);
            let [l_d, _] = process_sample(&mut dry, &ctx_dry);
            let [l_w, _] = process_sample(&mut wet, &ctx_wet);
            assert!(l_d.is_finite(), "dry NaN at {i}");
            assert!(l_w.is_finite(), "wet NaN at {i}");
            if i > 500 {
                diff += (l_d - l_w).abs();
            }
        }
        assert!(diff > 0.5, "fm diff={diff}");
    }

    #[test]
    fn fm_index_mod_matrix_applies() {
        let bank = WavetableBank::factory_sine();
        let mut base = Patch::factory_fm_bell();
        base.mod_matrix.clear();
        base.lfo.depth = 0.0;
        base.lfo.target = "wt_position".into();
        let mut modded = base.clone();
        modded.mod_matrix.push(crate::patch::ModSlot {
            source: "lfo1".into(),
            target: "osc1_fm_index".into(),
            amount: 2.0,
            enabled: true,
        });
        modded.lfo.depth = 1.0;
        modded.lfo.rate = 10.0;
        modded.lfo.target = "wt_position".into();

        let mut v1 = VoiceState::new(&base);
        let mut v2 = VoiceState::new(&modded);
        let dt = 1.0 / 44100.0;
        let mut diff = 0.0f32;
        for i in 0..4410 {
            let t = i as f32 * dt;
            let [l1, _] = process_sample(
                &mut v1,
                &single_bank_ctx(&bank, &base, 660.0, true, 1.0, t, dt),
            );
            let [l2, _] = process_sample(
                &mut v2,
                &single_bank_ctx(&bank, &modded, 660.0, true, 1.0, t, dt),
            );
            assert!(l1.is_finite() && l2.is_finite());
            diff += (l1 - l2).abs();
        }
        assert!(diff > 0.01, "mod fm diff={diff}");
    }

    #[test]
    fn lfo2_mod_matrix_applies() {
        let bank = WavetableBank::factory_sine();
        let mut patch = Patch::default_mono();
        patch.lfo2.rate = 8.0;
        patch.lfo2.depth = 1.0;
        patch.mod_matrix.push(crate::patch::ModSlot {
            source: "lfo2".into(),
            target: "filter_cutoff".into(),
            amount: 0.5,
            enabled: true,
        });
        let mut dry = Patch::default_mono();
        dry.mod_matrix.clear();

        let mut v_wet = VoiceState::new(&patch);
        let mut v_dry = VoiceState::new(&dry);
        let dt = 1.0 / 44100.0;
        let mut diff = 0.0f32;
        for i in 0..4410 {
            let t = i as f32 * dt;
            let wet = process_sample(&mut v_wet, &single_bank_ctx(&bank, &patch, 440.0, true, 1.0, t, dt));
            let dry_s = process_sample(&mut v_dry, &single_bank_ctx(&bank, &dry, 440.0, true, 1.0, t, dt));
            diff += (wet[0] - dry_s[0]).abs();
        }
        assert!(diff > 0.1, "lfo2 mod diff={diff}");
    }

    #[test]
    fn mpe_pitch_bend_shifts_pitch() {
        let bank = WavetableBank::factory_sine();
        let patch = Patch::default_mono();
        let dt = 1.0 / 44100.0;
        let mut center = VoiceState::new(&patch);
        let mut bent = VoiceState::new(&patch);
        let mut diff = 0.0f32;
        for i in 0..4410 {
            let t = i as f32 * dt;
            let ctx_c = single_bank_ctx(&bank, &patch, 440.0, true, 1.0, t, dt);
            let mut ctx_b = single_bank_ctx(&bank, &patch, 440.0, true, 1.0, t, dt);
            ctx_b.mpe.pitch_bend = 0.5;
            let [l_c, _] = process_sample(&mut center, &ctx_c);
            let [l_b, _] = process_sample(&mut bent, &ctx_b);
            if i > 500 {
                diff += (l_c - l_b).abs();
            }
        }
        assert!(diff > 0.01, "mpe bend diff={diff}");
    }

    #[test]
    fn macro_changes_cutoff() {
        let bank = WavetableBank::factory_saw_morph();
        let mut patch = Patch::default_mono();
        patch.macros[0].value = 1.0;
        patch.macros[0].target = "filter_cutoff".into();
        patch.macros[0].amount = 1.0;
        let mut dry = patch.clone();
        dry.macros[0].value = 0.0;

        let mut v_wet = VoiceState::new(&patch);
        let mut v_dry = VoiceState::new(&dry);
        let dt = 1.0 / 44100.0;
        let mut diff = 0.0f32;
        for i in 0..4410 {
            let t = i as f32 * dt;
            let wet = process_sample(&mut v_wet, &single_bank_ctx(&bank, &patch, 440.0, true, 1.0, t, dt));
            let dry_s = process_sample(&mut v_dry, &single_bank_ctx(&bank, &dry, 440.0, true, 1.0, t, dt));
            diff += (wet[0] - dry_s[0]).abs();
        }
        assert!(diff > 0.1, "macro diff={diff}");
    }
