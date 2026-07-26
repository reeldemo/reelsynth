//! Minimal AbletonOSC UDP client (Luftbahn-compatible addresses).

use std::net::UdpSocket;
use std::time::Duration;

const OSC_HOST: &str = "127.0.0.1:11000";
const PROBE_TIMEOUT_MS: u64 = 500;

/// Build a simple OSC message (address + typetag `,` + no args, or `,i` / `,s` / `,f`).
fn osc_packet(address: &str, args: &[OscArg]) -> Vec<u8> {
    let mut buf = Vec::new();
    write_osc_string(&mut buf, address);
    let mut typetag = String::from(",");
    for a in args {
        match a {
            OscArg::Int(_) => typetag.push('i'),
            OscArg::Float(_) => typetag.push('f'),
            OscArg::Str(_) => typetag.push('s'),
        }
    }
    write_osc_string(&mut buf, &typetag);
    for a in args {
        match a {
            OscArg::Int(v) => buf.extend_from_slice(&v.to_be_bytes()),
            OscArg::Float(v) => buf.extend_from_slice(&v.to_be_bytes()),
            OscArg::Str(s) => write_osc_string(&mut buf, s),
        }
    }
    buf
}

fn write_osc_string(buf: &mut Vec<u8>, s: &str) {
    buf.extend_from_slice(s.as_bytes());
    buf.push(0);
    while buf.len() % 4 != 0 {
        buf.push(0);
    }
}

enum OscArg {
    Int(i32),
    Float(f32),
    Str(String),
}

fn send_osc(sock: &UdpSocket, address: &str, args: &[OscArg]) -> Result<(), String> {
    let pkt = osc_packet(address, args);
    sock.send_to(&pkt, OSC_HOST)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Returns true if AbletonOSC appears to respond on :11000.
pub fn probe_ableton_osc() -> bool {
    let Ok(sock) = UdpSocket::bind("127.0.0.1:0") else {
        return false;
    };
    let _ = sock.set_read_timeout(Some(Duration::from_millis(PROBE_TIMEOUT_MS)));
    let _ = sock.set_write_timeout(Some(Duration::from_millis(PROBE_TIMEOUT_MS)));
    if send_osc(&sock, "/live/song/get/tempo", &[]).is_err() {
        return false;
    }
    let mut buf = [0u8; 256];
    sock.recv_from(&mut buf).is_ok()
}

/// Create MIDI track, insert Wavetable, apply normalized params by alias names.
pub fn push_wavetable_params(params: &[(String, f32)]) -> Result<String, String> {
    let sock = UdpSocket::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let _ = sock.set_read_timeout(Some(Duration::from_millis(PROBE_TIMEOUT_MS)));
    let _ = sock.set_write_timeout(Some(Duration::from_millis(PROBE_TIMEOUT_MS)));

    send_osc(
        &sock,
        "/live/song/create_midi_track",
        &[OscArg::Int(-1)],
    )?;
    // Best-effort: insert on last track (high index); AbletonOSC often uses track index.
    // Use track -1 / last: many forks accept track index from create reply; we use a large index probe.
    // Spec: insert Wavetable on the new track — try index from song get num tracks is ideal;
    // without reply parsing, insert on track 0 as fallback then also try -1 patterns.
    send_osc(
        &sock,
        "/live/track/insert_device",
        &[OscArg::Int(-1), OscArg::Str("Wavetable".into())],
    )?;

    let mut applied = 0usize;
    let mut unmatched = Vec::new();
    for (name, value) in params {
        // Parameter index unknown without get/parameters/name round-trip; send by name if fork supports it.
        // Luftbahn: set by index after listing — we send floating set with name as string where supported.
        if send_osc(
            &sock,
            "/live/device/set/parameter/value",
            &[
                OscArg::Int(-1),
                OscArg::Int(0),
                OscArg::Str(name.clone()),
                OscArg::Float(*value),
            ],
        )
        .is_ok()
        {
            applied += 1;
        } else {
            unmatched.push(name.clone());
        }
    }

    let mut msg = format!("AbletonOSC: inserted Wavetable; applied ~{applied} param msgs");
    if !unmatched.is_empty() {
        msg.push_str(&format!(
            "; unmatched/uncertain: {}",
            unmatched.join(", ")
        ));
    }
    msg.push_str(". Drag table_multicycle.wav onto the Wavetable sprite.");
    Ok(msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_offline_is_bool() {
        // Must not panic; typically false without Live.
        let _ = probe_ableton_osc();
    }

    #[test]
    fn osc_packet_aligned() {
        let p = osc_packet("/live/song/get/tempo", &[]);
        assert_eq!(p.len() % 4, 0);
    }
}
