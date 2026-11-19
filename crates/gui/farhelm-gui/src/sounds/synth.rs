//! Status sounds synthesized into small mono 16-bit WAV files in memory.
//!
//! The old app's three cues keep their original notes. The SSH cue uses the same oscillators, so the app can play
//! it through its existing platform player without a new audio file or crate.
//!
//! The old app played these through Web Audio (`assets/sounds.js` at the reference commit, `createPlayer`): per note,
//! one oscillator for the fundamental and, for a bell, two more at twice and three times its frequency, each through
//! its own gain envelope into a master gain of 0.3. This module replaces each of those oscillators one for one with
//! the same arithmetic over PCM samples, from the old `NOTES` table copied verbatim as data ([`notes`]). Here's the
//! TLDR of what Web Audio did with each oscillator, and what [`render`] reproduces:
//!
//! - The gain is set to 0.0001 at the note's start, ramps exponentially to the note's amplitude (times the overtone's
//!   relative level) over 5 ms, then ramps exponentially back to 0.0001 at the note's start plus its duration.
//! - The oscillator starts at phase zero at the note's start and stops 50 ms after the decay ends, with the gain
//!   holding 0.0001 for those 50 ms (the release tail, essentially silent).
//! - Every oscillator is a sine, except a note marked triangle, whose oscillators are all Web Audio's band-limited
//!   triangle.
//!
//! The old player scheduled the first note 20 ms after the moment it was asked to play, which is scheduling slack,
//! not part of the sound, so the rendered files start at the first note. Each sound is rendered once per process, on
//! first use ([`wav`]).

use std::f64::consts::TAU;
use std::sync::OnceLock;

use super::Sound;

/// The rendered files' sample rate. 44.1 kHz keeps the highest partial (a bell's third harmonic at 3141 Hz) and the
/// triangle's audible harmonics far below the Nyquist frequency.
pub const SAMPLE_RATE: u32 = 44_100;

/// Web Audio's master gain in the old player: every sound is mixed at this level.
const MASTER: f64 = 0.3;

/// How long each oscillator's gain takes to ramp from [`FLOOR`] up to its peak, in seconds.
const ATTACK: f64 = 0.005;

/// The gain an envelope starts from and decays to. Web Audio's exponential ramps cannot reach zero, which is why the
/// old player used this value rather than silence.
const FLOOR: f64 = 0.0001;

/// How long an oscillator keeps running at [`FLOOR`] after its note's decay ends, in seconds: the old player stopped
/// each one at the note's start plus its duration plus 0.05.
const RELEASE: f64 = 0.05;

/// A bell's partials as (frequency ratio, relative level): the fundamental, then the old player's overtones at twice
/// and three times its frequency.
const BELL: [(f64, f64); 3] = [(1., 1.), (2., 0.25), (3., 0.08)];

/// A plain note's one partial: the fundamental alone.
const PLAIN: [(f64, f64); 1] = [(1., 1.)];

/// An oscillator's waveform: Web Audio's `type`, of which the old sounds use two.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Wave {
    Sine,
    Triangle,
}

/// One row of the old `NOTES` table. The field names are the old ones, so the table below reads like the original.
#[derive(Clone, Copy, Debug)]
struct Note {
    /// The fundamental's frequency in Hz.
    f: f64,
    /// When the note starts, in seconds from the sound's start.
    t: f64,
    /// How long the note sounds before its decay reaches [`FLOOR`], in seconds (the attack included).
    d: f64,
    /// The fundamental's peak gain, before [`MASTER`].
    a: f64,
    /// Whether the note adds the bell's overtones ([`BELL`]).
    bell: bool,
    /// The waveform of every partial; the old table left it out for a sine.
    wave: Wave,
}

/// The old `NOTES.waiting`: a rising two-note bell.
///
/// The three tables keep one note per line, as the old file does, so each row can be checked against it at a glance.
#[rustfmt::skip]
const WAITING: [Note; 2] = [
    Note { f: 784., t: 0., d: 0.7, a: 0.4, bell: true, wave: Wave::Sine },
    Note { f: 1047., t: 0.18, d: 0.9, a: 0.35, bell: true, wave: Wave::Sine },
];

/// The old `NOTES.approval`: a falling two-note ding-dong.
#[rustfmt::skip]
const APPROVAL: [Note; 2] = [
    Note { f: 880., t: 0., d: 0.6, a: 0.4, bell: true, wave: Wave::Sine },
    Note { f: 698., t: 0.25, d: 0.9, a: 0.4, bell: true, wave: Wave::Sine },
];

/// The old `NOTES.finished`: one soft triangle pluck.
#[rustfmt::skip]
const TURN_FINISHED: [Note; 1] = [
    Note { f: 523., t: 0., d: 0.35, a: 0.45, bell: false, wave: Wave::Triangle },
];

/// The SSH cue is a short rising pair, distinct from the longer session-waiting bell.
#[rustfmt::skip]
const SSH_SIGN_IN: [Note; 2] = [
    Note { f: 659., t: 0., d: 0.28, a: 0.4, bell: true, wave: Wave::Sine },
    Note { f: 988., t: 0.12, d: 0.38, a: 0.35, bell: true, wave: Wave::Sine },
];

/// The oscillator notes that define each cue.
fn notes(sound: Sound) -> &'static [Note] {
    match sound {
        Sound::SshSignIn => &SSH_SIGN_IN,
        Sound::Approval => &APPROVAL,
        Sound::Waiting => &WAITING,
        Sound::TurnFinished => &TURN_FINISHED,
    }
}

/// A note's partials as (frequency ratio, relative level).
fn partials(note: &Note) -> &'static [(f64, f64)] {
    if note.bell { &BELL } else { &PLAIN }
}

// ---- Rendering ------------------------------------------------------------------------------------------------------

/// `sound` as a complete mono 16-bit PCM WAV file, rendered on first use and kept for the life of the process.
///
/// The bytes never change, so a UI reset has no reason to render them again, and nothing here depends on which UI
/// session asks.
pub fn wav(sound: Sound) -> &'static [u8] {
    static SSH_SIGN_IN_WAV: OnceLock<Vec<u8>> = OnceLock::new();
    static APPROVAL_WAV: OnceLock<Vec<u8>> = OnceLock::new();
    static WAITING_WAV: OnceLock<Vec<u8>> = OnceLock::new();
    static TURN_FINISHED_WAV: OnceLock<Vec<u8>> = OnceLock::new();
    let cell = match sound {
        Sound::SshSignIn => &SSH_SIGN_IN_WAV,
        Sound::Approval => &APPROVAL_WAV,
        Sound::Waiting => &WAITING_WAV,
        Sound::TurnFinished => &TURN_FINISHED_WAV,
    };
    cell.get_or_init(|| render(sound))
}

/// Renders `sound` into a fresh WAV file. Deterministic: the same build renders the same bytes every time.
fn render(sound: Sound) -> Vec<u8> {
    encode_wav(&mix(notes(sound)))
}

/// How many samples a sound made of `notes` lasts: up to the end of its latest-ending note's release tail.
fn sample_count(notes: &[Note]) -> usize {
    let end = notes.iter().map(|note| note.t + note.d).fold(0., f64::max);
    ((end + RELEASE) * f64::from(SAMPLE_RATE)).round() as usize
}

/// Every partial of every note summed at [`MASTER`] level, as samples in -1..=1 (before quantizing).
///
/// Each partial contributes only while its oscillator would have run: from its note's start until the release tail
/// ends.
fn mix(notes: &[Note]) -> Vec<f64> {
    let rate = f64::from(SAMPLE_RATE);
    let mut out = vec![0.; sample_count(notes)];
    for note in notes {
        for &(ratio, relative) in partials(note) {
            let peak = note.a * relative;
            let oscillator = Oscillator::new(note.wave, note.f * ratio);
            // Only this note's own samples: from its start to the end of its release, clipped to the buffer.
            let first = (note.t * rate).ceil() as usize;
            let end = (((note.t + note.d + RELEASE) * rate).ceil() as usize).min(out.len());
            for (n, sample) in out.iter_mut().enumerate().take(end).skip(first) {
                let elapsed = n as f64 / rate - note.t;
                if (0. ..note.d + RELEASE).contains(&elapsed) {
                    *sample += MASTER * envelope(peak, note.d, elapsed) * oscillator.at(elapsed);
                }
            }
        }
    }
    out
}

/// The gain Web Audio's automation gives an oscillator `elapsed` seconds after its note starts: an exponential ramp
/// from [`FLOOR`] to `peak` over [`ATTACK`], an exponential ramp back to [`FLOOR`] at `duration`, then [`FLOOR`].
///
/// An exponential ramp from `v0` at `t0` to `v1` at `t1` is `v0 * (v1 / v0)^((t - t0) / (t1 - t0))`, the formula the
/// Web Audio specification gives for `exponentialRampToValueAtTime`.
fn envelope(peak: f64, duration: f64, elapsed: f64) -> f64 {
    if elapsed < ATTACK {
        FLOOR * (peak / FLOOR).powf(elapsed / ATTACK)
    } else if elapsed < duration {
        peak * (FLOOR / peak).powf((elapsed - ATTACK) / (duration - ATTACK))
    } else {
        FLOOR
    }
}

/// One oscillator at a fixed frequency, starting at phase zero, with a peak of 1.
enum Oscillator {
    Sine {
        frequency: f64,
    },
    /// A triangle built the way Web Audio builds its built-in one: the triangle's Fourier series (odd harmonics `k`,
    /// sine terms of weight `1/k²`, alternating in sign, so it rises from zero like a sine), keeping only harmonics
    /// below the Nyquist frequency so none alias, scaled so the peak is exactly 1. A browser's cutoff may sit
    /// somewhere else, but the harmonics near the Nyquist frequency each weigh under a thousandth of the fundamental.
    Triangle {
        frequency: f64,
        /// Each kept harmonic's number and signed weight, already divided by the scale that makes the peak 1.
        harmonics: Vec<(f64, f64)>,
    },
}

impl Oscillator {
    /// An oscillator of `wave` at `frequency` Hz.
    fn new(wave: Wave, frequency: f64) -> Oscillator {
        match wave {
            Wave::Sine => Oscillator::Sine { frequency },
            Wave::Triangle => {
                let nyquist = f64::from(SAMPLE_RATE) / 2.;
                let terms: Vec<(f64, f64)> = (1u32..)
                    .step_by(2)
                    .take_while(|&k| f64::from(k) * frequency < nyquist)
                    .map(|k| {
                        let sign = if k % 4 == 1 { 1. } else { -1. };
                        let k = f64::from(k);
                        (k, sign / (k * k))
                    })
                    .collect();
                // Every term peaks together at a quarter period, so the sum of the weights' magnitudes is the peak.
                let scale: f64 = terms.iter().map(|(_, weight)| weight.abs()).sum();
                let harmonics = terms
                    .into_iter()
                    .map(|(k, weight)| (k, weight / scale))
                    .collect();
                Oscillator::Triangle {
                    frequency,
                    harmonics,
                }
            }
        }
    }

    /// The oscillator's value `elapsed` seconds after it started.
    fn at(&self, elapsed: f64) -> f64 {
        match self {
            Oscillator::Sine { frequency } => (TAU * frequency * elapsed).sin(),
            Oscillator::Triangle {
                frequency,
                harmonics,
            } => {
                let phase = TAU * frequency * elapsed;
                harmonics
                    .iter()
                    .map(|(k, weight)| weight * (k * phase).sin())
                    .sum()
            }
        }
    }
}

// ---- The WAV container ----------------------------------------------------------------------------------------------

/// The size of the canonical 44-byte header [`encode_wav`] writes: a RIFF chunk holding a 16-byte `fmt ` chunk and
/// the `data` chunk's own header.
const HEADER_LEN: usize = 44;

/// `samples` (in -1..=1) as a mono 16-bit little-endian PCM WAV file, the plainest form every platform's player takes.
fn encode_wav(samples: &[f64]) -> Vec<u8> {
    let data_len = samples.len() * 2;
    let mut out = Vec::with_capacity(HEADER_LEN + data_len);
    // A sound is about a second long, so these lengths are far below u32's range and never saturate; saturating rather
    // than unwrapping only satisfies the GUI crates' ban on `unwrap`.
    let riff_len = u32::try_from(HEADER_LEN - 8 + data_len).unwrap_or(u32::MAX);
    let data_len32 = u32::try_from(data_len).unwrap_or(u32::MAX);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_len.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // bytes per second
    out.extend_from_slice(&2u16.to_le_bytes()); // bytes per sample frame
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len32.to_le_bytes());
    for &sample in samples {
        out.extend_from_slice(&quantize(sample).to_le_bytes());
    }
    out
}

/// A sample in -1..=1 as a 16-bit PCM value, clamped rather than wrapped should a mix ever exceed full scale.
fn quantize(sample: f64) -> i16 {
    let max = f64::from(i16::MAX);
    (sample * max).round().clamp(f64::from(i16::MIN), max) as i16
}

// ---- Tests ----------------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    //! Cheap properties of the rendered files only. Whether the sounds sound right is judged by ear on a devbuild, and
    //! whether the table matches the old one by review, so nothing here analyzes a spectrum or an envelope.

    use super::*;

    /// The cues exercised by the format and render checks.
    const ALL: [Sound; 4] = Sound::PRIORITY;

    /// The little-endian u16 at `at`.
    fn u16_at(bytes: &[u8], at: usize) -> u16 {
        u16::from_le_bytes([bytes[at], bytes[at + 1]])
    }

    /// The little-endian u32 at `at`.
    fn u32_at(bytes: &[u8], at: usize) -> u32 {
        u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
    }

    /// The file's samples, after its 44-byte header.
    fn samples(bytes: &[u8]) -> Vec<i16> {
        let (pairs, odd) = bytes[HEADER_LEN..].as_chunks::<2>();
        assert!(odd.is_empty(), "the data chunk holds whole 16-bit samples");
        pairs.iter().map(|&pair| i16::from_le_bytes(pair)).collect()
    }

    /// A playback seam hands these bytes to a platform player as a file, so the header must be a well-formed mono
    /// 16-bit PCM WAV whose lengths agree with the file: a player may refuse or truncate a file whose header lies.
    #[test]
    fn each_sound_is_a_valid_mono_pcm_wav() {
        for sound in ALL {
            let bytes = wav(sound);
            assert_eq!(&bytes[0..4], b"RIFF", "{sound:?}");
            assert_eq!(
                u32_at(bytes, 4) as usize,
                bytes.len() - 8,
                "{sound:?} RIFF length"
            );
            assert_eq!(&bytes[8..12], b"WAVE", "{sound:?}");
            assert_eq!(&bytes[12..16], b"fmt ", "{sound:?}");
            assert_eq!(u32_at(bytes, 16), 16, "{sound:?} fmt length");
            assert_eq!(u16_at(bytes, 20), 1, "{sound:?} PCM");
            assert_eq!(u16_at(bytes, 22), 1, "{sound:?} mono");
            assert_eq!(u32_at(bytes, 24), SAMPLE_RATE, "{sound:?} sample rate");
            assert_eq!(u32_at(bytes, 28), SAMPLE_RATE * 2, "{sound:?} byte rate");
            assert_eq!(u16_at(bytes, 32), 2, "{sound:?} block align");
            assert_eq!(u16_at(bytes, 34), 16, "{sound:?} bits");
            assert_eq!(&bytes[36..40], b"data", "{sound:?}");
            assert_eq!(
                u32_at(bytes, 40) as usize,
                bytes.len() - HEADER_LEN,
                "{sound:?} data length"
            );
        }
    }

    /// Each sound lasts until its latest note's start plus duration, plus the 50 ms release tail, as the old player's
    /// oscillators did. The ends are written out from the old table rather than computed from the copy, so a slip in
    /// a start time or duration shows up here too.
    #[test]
    fn each_sound_lasts_until_its_release_tail_ends() {
        let ends = [
            (Sound::SshSignIn, 0.12 + 0.38),
            (Sound::Approval, 0.25 + 0.9),
            (Sound::Waiting, 0.18 + 0.9),
            (Sound::TurnFinished, 0.35),
        ];
        for (sound, end) in ends {
            let expected = ((end + 0.05) * f64::from(SAMPLE_RATE)).round() as usize;
            assert_eq!(samples(wav(sound)).len(), expected, "{sound:?}");
        }
    }

    /// The mix can never exceed the master level times the sum of every partial's peak, which is what keeps the
    /// sounds at the old volume and out of clipping; a sound far below that would be inaudible, so each must also
    /// reach a clearly audible level.
    #[test]
    fn each_sound_peaks_within_its_bound_and_is_not_silent() {
        for sound in ALL {
            let bound: f64 = MASTER
                * notes(sound)
                    .iter()
                    .flat_map(|note| partials(note).iter().map(|(_, relative)| note.a * relative))
                    .sum::<f64>();
            let peak = samples(wav(sound))
                .iter()
                .map(|sample| f64::from(sample.unsigned_abs()))
                .fold(0., f64::max);
            // One quantization step of slack: the bound is in real numbers, the samples are rounded.
            assert!(
                peak <= bound * f64::from(i16::MAX) + 1.,
                "{sound:?}: peak {peak}, bound {bound}"
            );
            assert!(
                peak > f64::from(i16::MAX) / 100.,
                "{sound:?}: peak {peak} is too quiet"
            );
        }
    }

    /// Rendering involves no clock and no randomness, so a sound renders to the same bytes every time, and the cached
    /// file is that rendering: a sound must not vary between plays.
    #[test]
    fn rendering_is_deterministic() {
        for sound in ALL {
            assert_eq!(render(sound), render(sound), "{sound:?}");
            assert_eq!(wav(sound), render(sound).as_slice(), "{sound:?}");
        }
    }
}
