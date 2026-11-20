//! G.711 μ-law helpers. Twilio Media Streams carry 8 kHz mono μ-law audio.
//! The server itself only forwards bytes; encoding is used by the fake client.

pub const SAMPLE_RATE: u32 = 8000;
/// Twilio sends 20 ms of audio per media message: 160 bytes at 8 kHz.
pub const FRAME_BYTES: usize = 160;
pub const FRAME_MS: u64 = 20;

const BIAS: i32 = 0x84;
const CLIP: i32 = 32635;

/// Encodes one 16-bit PCM sample as μ-law.
pub fn encode(sample: i16) -> u8 {
    let mut magnitude = i32::from(sample);
    let sign = if magnitude < 0 {
        magnitude = -magnitude;
        0x80
    } else {
        0
    };
    magnitude = magnitude.min(CLIP) + BIAS;

    let mut exponent = 7;
    let mut mask = 0x4000;
    while magnitude & mask == 0 && exponent > 0 {
        exponent -= 1;
        mask >>= 1;
    }
    let mantissa = (magnitude >> (exponent + 3)) & 0x0F;
    !(sign | (exponent << 4) | mantissa) as u8
}

/// Decodes one μ-law byte to 16-bit PCM.
pub fn decode(byte: u8) -> i16 {
    let value = !byte;
    let exponent = i32::from((value >> 4) & 0x07);
    let mantissa = i32::from(value & 0x0F);
    let magnitude = (((mantissa << 3) + BIAS) << exponent) - BIAS;
    if value & 0x80 != 0 {
        -magnitude as i16
    } else {
        magnitude as i16
    }
}

/// μ-law encoded sine tone, useful as a stand-in for speech.
pub fn tone(frequency_hz: f32, duration_ms: u64, amplitude: f32) -> Vec<u8> {
    let samples = (u64::from(SAMPLE_RATE) * duration_ms / 1000) as usize;
    (0..samples)
        .map(|n| {
            let t = n as f32 / SAMPLE_RATE as f32;
            let value = (2.0 * std::f32::consts::PI * frequency_hz * t).sin()
                * amplitude
                * f32::from(i16::MAX);
            encode(value as i16)
        })
        .collect()
}

/// μ-law encoded silence.
pub fn silence(duration_ms: u64) -> Vec<u8> {
    vec![encode(0); (u64::from(SAMPLE_RATE) * duration_ms / 1000) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_is_0xff() {
        assert_eq!(encode(0), 0xFF);
        assert_eq!(decode(0xFF), 0);
    }

    #[test]
    fn round_trip_is_close() {
        for sample in [-32000i16, -1000, -10, 10, 1000, 8000, 32000] {
            let back = decode(encode(sample));
            let tolerance = (i32::from(sample).abs() / 16).max(8);
            assert!(
                (i32::from(back) - i32::from(sample)).abs() <= tolerance,
                "{sample} -> {back}"
            );
        }
    }

    #[test]
    fn tone_has_expected_length() {
        assert_eq!(tone(440.0, 20, 0.5).len(), FRAME_BYTES);
        assert_eq!(silence(1000).len(), 8000);
    }
}
