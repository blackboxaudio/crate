use std::time::Duration;

use rodio::Source;

/// Duration of the fade-out ramp applied at the end of a track to prevent clicks.
const FADE_OUT_DURATION_MS: u64 = 5;

/// A `rodio::Source` wrapper that applies a short linear fade-out at the very
/// end of playback. All samples pass through unchanged until the final
/// [`FADE_OUT_DURATION_MS`] milliseconds, which ramp linearly from 1.0 to 0.0.
///
/// This eliminates the audible click caused by an instantaneous amplitude drop
/// when a track finishes.
pub struct FadeOutEnding<S> {
    inner: S,
    /// Total samples yielded so far (one per channel per frame).
    samples_yielded: u64,
    /// Samples per millisecond (sample_rate * channels / 1000).
    samples_per_ms: u64,
    /// The sample index at which the fade region begins.
    fade_start_sample: u64,
    /// Total number of samples in the fade region.
    fade_length: u64,
}

impl<S> FadeOutEnding<S>
where
    S: Source<Item = f32>,
{
    /// Wraps `inner` so that the last [`FADE_OUT_DURATION_MS`] ms fade to silence.
    ///
    /// `duration_ms` is the total track duration obtained from metadata. When it
    /// is zero the wrapper becomes a pure pass-through (no fading).
    pub fn new(inner: S, duration_ms: u64) -> Self {
        let sample_rate = inner.sample_rate() as u64;
        let channels = inner.channels() as u64;
        let samples_per_ms = sample_rate * channels / 1000;
        let total_samples = duration_ms * samples_per_ms;

        let fade_ms = FADE_OUT_DURATION_MS.min(duration_ms);
        let fade_length = fade_ms * samples_per_ms;
        let fade_start_sample = total_samples.saturating_sub(fade_length);

        Self {
            inner,
            samples_yielded: 0,
            samples_per_ms,
            fade_start_sample,
            fade_length,
        }
    }
}

impl<S> Iterator for FadeOutEnding<S>
where
    S: Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let sample = self.inner.next()?;
        let idx = self.samples_yielded;
        self.samples_yielded += 1;

        if self.fade_length == 0 || idx < self.fade_start_sample {
            return Some(sample);
        }

        let fade_position = idx - self.fade_start_sample;
        let scale = 1.0 - (fade_position as f32 / self.fade_length as f32);
        Some(sample * scale.max(0.0))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<S> Source for FadeOutEnding<S>
where
    S: Source<Item = f32>,
{
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }

    fn channels(&self) -> u16 {
        self.inner.channels()
    }

    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
        let result = self.inner.try_seek(pos);
        if result.is_ok() {
            let pos_ms = pos.as_millis() as u64;
            self.samples_yielded = pos_ms * self.samples_per_ms;
        }
        result
    }
}
