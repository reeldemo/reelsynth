//! Automation lane evaluation during playback.

use std::collections::HashMap;

use crate::modulation::apply_target_scale;

use super::schema::{AutomationPoint, Clip, SequenceProject, Track};
use super::transport::TransportState;

/// Interpolate automation value at `beat` (0..1).
pub fn evaluate_lane(points: &[AutomationPoint], beat: f32) -> f32 {
    if points.is_empty() {
        return 0.5;
    }
    if points.len() == 1 {
        return points[0].value.clamp(0.0, 1.0);
    }
    let mut sorted = points.to_vec();
    sorted.sort_by(|a, b| a.beats.partial_cmp(&b.beats).unwrap_or(std::cmp::Ordering::Equal));

    if beat <= sorted[0].beats {
        return sorted[0].value.clamp(0.0, 1.0);
    }
    if beat >= sorted[sorted.len() - 1].beats {
        return sorted[sorted.len() - 1].value.clamp(0.0, 1.0);
    }

    for window in sorted.windows(2) {
        let a = &window[0];
        let b = &window[1];
        if beat >= a.beats && beat <= b.beats {
            let t = if (b.beats - a.beats).abs() < 1e-6 {
                0.0
            } else {
                (beat - a.beats) / (b.beats - a.beats)
            };
            return (a.value + t * (b.value - a.value)).clamp(0.0, 1.0);
        }
    }
    0.5
}

/// Sum automation lanes active at playhead into mod-target offsets.
pub fn compute_automation_mods(
    project: &SequenceProject,
    transport: &TransportState,
) -> HashMap<String, f32> {
    if !transport.playing {
        return HashMap::new();
    }
    let playhead = transport.playhead_beats;
    let solo_any = project.tracks.iter().any(|t| t.solo);
    let mut out = HashMap::new();

    for track in &project.tracks {
        if track.mute || (solo_any && !track.solo) {
            continue;
        }
        for clip in clips_at_playhead(track, playhead) {
            for lane in &clip.automation {
                let value = evaluate_lane(&lane.points, playhead - clip.start_beats);
                let centered = (value - 0.5) * 2.0;
                let delta = apply_target_scale(&lane.target, centered, 1.0);
                *out.entry(lane.target.clone()).or_insert(0.0) += delta;
            }
        }
    }
    out
}

fn clips_at_playhead<'a>(track: &'a Track, playhead: f32) -> Vec<&'a Clip> {
    track
        .clips
        .iter()
        .filter(|c| playhead >= c.start_beats && playhead < c.start_beats + c.length_beats)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_interpolation() {
        let points = vec![
            AutomationPoint {
                beats: 0.0,
                value: 0.0,
            },
            AutomationPoint {
                beats: 2.0,
                value: 1.0,
            },
        ];
        assert!((evaluate_lane(&points, 1.0) - 0.5).abs() < 1e-5);
    }
}
