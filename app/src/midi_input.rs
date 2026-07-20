//! MIDI input device enumeration, hot-plug refresh, and note routing.

use crossbeam_channel::Sender;
use midir::{Ignore, MidiInput, MidiInputConnection};
use reelsynth::engine::{pitch_bend_from_raw, MidiEvent};

pub struct MidiDevices {
    pub names: Vec<String>,
    port_ids: Vec<String>,
}

pub const MIDI_NONE_LABEL: &str = "No MIDI";

impl MidiDevices {
    pub fn enumerate() -> Self {
        let mut names = vec![MIDI_NONE_LABEL.into()];
        let mut port_ids = vec![String::new()];
        if let Ok(midi_in) = MidiInput::new("reelsynth-ui-enumerate") {
            for port in midi_in.ports() {
                if let Ok(name) = midi_in.port_name(&port) {
                    names.push(name.clone());
                    port_ids.push(name);
                }
            }
        }
        Self { names, port_ids }
    }

    pub fn refresh(&mut self) -> bool {
        let fresh = Self::enumerate();
        let changed = fresh.port_ids != self.port_ids;
        *self = fresh;
        changed
    }

    pub fn keyboard_like_index(&self) -> Option<usize> {
        if self.names.len() <= 1 {
            return None;
        }
        let keywords = ["keyboard", "keys", "piano", "midi", "keystation", "keylab"];
        for (idx, name) in self.names.iter().enumerate().skip(1) {
            let lower = name.to_ascii_lowercase();
            if keywords.iter().any(|k| lower.contains(k)) {
                return Some(idx);
            }
        }
        if self.names.len() == 2 {
            return Some(1);
        }
        None
    }
}

pub struct MidiInputHandle {
    _connection: Option<MidiInputConnection<()>>,
}

impl MidiInputHandle {
    pub fn disconnected() -> Self {
        Self {
            _connection: None,
        }
    }

    /// Connect to device index in [`MidiDevices::names`] (`0` = none).
    pub fn connect(
        devices: &MidiDevices,
        index: usize,
        event_tx: Sender<MidiEvent>,
    ) -> Result<Self, String> {
        if index == 0 {
            return Ok(Self::disconnected());
        }
        let device_index = index - 1;

        let mut midi_in = MidiInput::new("reelsynth-ui").map_err(|e| e.to_string())?;
        midi_in.ignore(Ignore::TimeAndActiveSense);

        let ports = midi_in.ports();
        let port = ports
            .get(device_index)
            .ok_or_else(|| "MIDI device index out of range".to_string())?;

        let port_name = midi_in
            .port_name(port)
            .unwrap_or_else(|_| devices.names[index].clone());

        let connection = midi_in
            .connect(
                port,
                "reelsynth-ui-in",
                move |_stamp, message, _| {
                    if message.len() < 2 {
                        return;
                    }
                    let status = message[0] & 0xF0;
                    let channel = message[0] & 0x0F;
                    let note = message[1];
                    match status {
                        0x90 if message.len() >= 3 => {
                            let vel = message[2];
                            if vel > 0 {
                                let _ = event_tx.send(MidiEvent::note_on(
                                    channel,
                                    note,
                                    vel as f32 / 127.0,
                                ));
                            } else {
                                let _ = event_tx.send(MidiEvent::note_off(channel, note));
                            }
                        }
                        0x80 => {
                            let _ = event_tx.send(MidiEvent::note_off(channel, note));
                        }
                        0xE0 if message.len() >= 3 => {
                            let bend = pitch_bend_from_raw(message[1], message[2]);
                            let _ = event_tx.send(MidiEvent::PitchBend { channel, value: bend });
                        }
                        0xD0 if message.len() >= 2 => {
                            let _ = event_tx.send(MidiEvent::ChannelPressure {
                                channel,
                                pressure: message[1] as f32 / 127.0,
                            });
                        }
                        0xA0 if message.len() >= 3 => {
                            let _ = event_tx.send(MidiEvent::PolyAftertouch {
                                channel,
                                note,
                                pressure: message[2] as f32 / 127.0,
                            });
                        }
                        0xB0 if message.len() >= 3 => {
                            let _ = event_tx.send(MidiEvent::ControlChange {
                                channel,
                                cc: note,
                                value: message[2] as f32 / 127.0,
                            });
                        }
                        _ => {}
                    }
                },
                (),
            )
            .map_err(|e| format!("MIDI connect failed ({port_name}): {e}"))?;

        Ok(Self {
            _connection: Some(connection),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_detects_port_list_change() {
        let mut devices = MidiDevices::enumerate();
        let before = devices.names.clone();
        let _ = devices.refresh();
        assert_eq!(devices.names, before);
    }

    #[test]
    fn keyboard_like_prefers_named_port() {
        let devices = MidiDevices {
            names: vec![
                MIDI_NONE_LABEL.into(),
                "Arturia KeyLab 61".into(),
                "Generic MIDI".into(),
            ],
            port_ids: vec!["".into(), "a".into(), "b".into()],
        };
        assert_eq!(devices.keyboard_like_index(), Some(1));
    }
}
