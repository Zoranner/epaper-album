pub const SELF_TEST_TONE_SAMPLE_RATE_HZ: u32 = 16_000;
pub const SELF_TEST_TONE_CHANNELS: usize = 2;
pub const SELF_TEST_TONE_BITS_PER_SAMPLE: usize = 16;

const SELF_TEST_TONE_AMPLITUDE: i16 = 8_000;
const SELF_TEST_TONE_SEGMENTS: [ToneSegment; 3] = [
    ToneSegment {
        frequency_hz: 880,
        duration_ms: 80,
    },
    ToneSegment {
        frequency_hz: 0,
        duration_ms: 40,
    },
    ToneSegment {
        frequency_hz: 1_320,
        duration_ms: 80,
    },
];

#[derive(Clone, Copy)]
struct ToneSegment {
    frequency_hz: u32,
    duration_ms: u32,
}

pub fn self_test_tone_pcm() -> Vec<u8> {
    let mut pcm = Vec::with_capacity(self_test_tone_pcm_len());
    for segment in SELF_TEST_TONE_SEGMENTS {
        append_tone_segment(&mut pcm, segment);
    }
    pcm
}

pub const fn self_test_tone_pcm_len() -> usize {
    let mut samples = 0usize;
    let mut index = 0usize;
    while index < SELF_TEST_TONE_SEGMENTS.len() {
        samples += segment_sample_count(SELF_TEST_TONE_SEGMENTS[index].duration_ms);
        index += 1;
    }

    samples * SELF_TEST_TONE_CHANNELS * core::mem::size_of::<i16>()
}

const fn segment_sample_count(duration_ms: u32) -> usize {
    (SELF_TEST_TONE_SAMPLE_RATE_HZ as usize * duration_ms as usize) / 1_000
}

fn append_tone_segment(pcm: &mut Vec<u8>, segment: ToneSegment) {
    let samples = segment_sample_count(segment.duration_ms);
    for sample_index in 0..samples {
        let sample = if segment.frequency_hz == 0 {
            0
        } else {
            square_wave_sample(sample_index, segment.frequency_hz)
        };
        append_stereo_sample(pcm, sample);
    }
}

fn square_wave_sample(sample_index: usize, frequency_hz: u32) -> i16 {
    let period = (SELF_TEST_TONE_SAMPLE_RATE_HZ / frequency_hz).max(1) as usize;
    if sample_index % period < period / 2 {
        SELF_TEST_TONE_AMPLITUDE
    } else {
        -SELF_TEST_TONE_AMPLITUDE
    }
}

fn append_stereo_sample(pcm: &mut Vec<u8>, sample: i16) {
    let bytes = sample.to_le_bytes();
    for _ in 0..SELF_TEST_TONE_CHANNELS {
        pcm.extend_from_slice(&bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_test_tone_pcm_has_expected_stereo_16bit_length() {
        let pcm = self_test_tone_pcm();

        assert_eq!(pcm.len(), self_test_tone_pcm_len());
        assert_eq!(pcm.len(), 12_800);
        assert_eq!(pcm.len() % (SELF_TEST_TONE_CHANNELS * 2), 0);
    }

    #[test]
    fn self_test_tone_pcm_contains_two_tones_with_silence_between() {
        let pcm = self_test_tone_pcm();
        let first_sample = i16::from_le_bytes([pcm[0], pcm[1]]);
        let pause_start = 80 * SELF_TEST_TONE_SAMPLE_RATE_HZ as usize / 1_000;
        let pause_byte = pause_start * SELF_TEST_TONE_CHANNELS * 2;
        let second_start = 120 * SELF_TEST_TONE_SAMPLE_RATE_HZ as usize / 1_000;
        let second_byte = second_start * SELF_TEST_TONE_CHANNELS * 2;

        assert_ne!(first_sample, 0);
        assert!(pcm[pause_byte..pause_byte + 160]
            .iter()
            .all(|byte| *byte == 0));
        assert_ne!(
            i16::from_le_bytes([pcm[second_byte], pcm[second_byte + 1]]),
            0
        );
    }
}
