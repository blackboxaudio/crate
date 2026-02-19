use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rodio::Source;

/// Duration of the fade-out ramp applied at the end of a track to prevent clicks.
const FADE_OUT_DURATION_MS: u64 = 5;

/// Duration of the pause/resume fade ramp in seconds.
const PAUSE_FADE_SECS: f32 = 0.005;

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

// ---------------------------------------------------------------------------
// PauseFade — source-level fade for click-free pause/resume
// ---------------------------------------------------------------------------

/// Atomic fade state shared between the audio source and the command handler.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FadeState {
    /// Samples pass through at full amplitude.
    Playing = 0,
    /// Ramping 1.0 → 0.0 over [`PAUSE_FADE_SECS`].
    FadingOut = 1,
    /// Output silence (fade-out finished, waiting for `sink.pause()`).
    Silent = 2,
    /// Ramping 0.0 → 1.0 over [`PAUSE_FADE_SECS`].
    FadingIn = 3,
}

impl FadeState {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Playing,
            1 => Self::FadingOut,
            2 => Self::Silent,
            3 => Self::FadingIn,
            _ => Self::Playing,
        }
    }
}

/// A `rodio::Source` wrapper that applies sample-level fades for click-free
/// pause and resume. The fade direction is controlled by a shared [`AtomicU8`]
/// which the command handler writes and the audio callback reads.
pub struct PauseFade<S> {
    inner: S,
    fade_state: Arc<AtomicU8>,
    amplitude: f32,
    /// Per-sample amplitude step: `1.0 / (sample_rate * channels * PAUSE_FADE_SECS)`.
    fade_step: f32,
}

impl<S> PauseFade<S>
where
    S: Source<Item = f32>,
{
    pub fn new(inner: S, fade_state: Arc<AtomicU8>) -> Self {
        let sample_rate = inner.sample_rate() as f32;
        let channels = inner.channels() as f32;
        let fade_step = 1.0 / (sample_rate * channels * PAUSE_FADE_SECS);

        Self {
            inner,
            fade_state,
            amplitude: 1.0,
            fade_step,
        }
    }
}

impl<S> Iterator for PauseFade<S>
where
    S: Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let sample = self.inner.next()?;
        let state = FadeState::from_u8(self.fade_state.load(Ordering::Relaxed));

        match state {
            FadeState::Playing => {
                self.amplitude = 1.0;
                Some(sample)
            }
            FadeState::FadingOut => {
                self.amplitude = (self.amplitude - self.fade_step).max(0.0);
                let out = sample * self.amplitude;
                if self.amplitude <= 0.0 {
                    self.fade_state
                        .store(FadeState::Silent as u8, Ordering::Relaxed);
                }
                Some(out)
            }
            FadeState::Silent => Some(0.0),
            FadeState::FadingIn => {
                self.amplitude = (self.amplitude + self.fade_step).min(1.0);
                let out = sample * self.amplitude;
                if self.amplitude >= 1.0 {
                    self.fade_state
                        .store(FadeState::Playing as u8, Ordering::Relaxed);
                }
                Some(out)
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<S> Source for PauseFade<S>
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
            self.amplitude = 0.0;
            self.fade_state
                .store(FadeState::FadingIn as u8, Ordering::Relaxed);
        }
        result
    }
}
