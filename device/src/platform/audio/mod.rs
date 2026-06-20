mod tone;

#[cfg(target_os = "espidf")]
pub mod espidf;

pub use tone::{
    self_test_tone_pcm, self_test_tone_pcm_len, SELF_TEST_TONE_BITS_PER_SAMPLE,
    SELF_TEST_TONE_CHANNELS, SELF_TEST_TONE_SAMPLE_RATE_HZ,
};
