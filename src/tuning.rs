// A pitch class is a f32 representing the number of cents mod 1200.

use std::{
    fmt::{self, Display},
    ops::{Add, Neg, Sub},
};

use nih_plug::nih_dbg;

// Just tunings for primes 3, 5, and 7
pub const THREE_JUST_F32: f32 = 701.955001;
pub const FIVE_JUST_F32: f32 = 386.313714;
pub const SEVEN_JUST_F32: f32 = 968.825906;

// 12TET approximations for primes 3, 5, and 7
pub const THREE_12TET_F32: f32 = 700.0;
pub const FIVE_12TET_F32: f32 = 400.0;
pub const SEVEN_12TET_F32: f32 = 1000.0;

pub const THREE_JUST: PitchClass = PitchClass::from_microcents(701_955_001);
pub const FIVE_JUST: PitchClass = PitchClass::from_microcents(386_313_714);
pub const SEVEN_JUST: PitchClass = PitchClass::from_microcents(968_825_906);

pub const CENTS_TO_MICROCENTS: u32 = 1_000_000;
const MIDI_NOTE_TO_CENTS: u32 = 100;
const OCTAVE_MICROCENTS: u32 = 1_200 * CENTS_TO_MICROCENTS;

const MIDI_NOTE_TO_CENTS_F32: f32 = MIDI_NOTE_TO_CENTS as f32;
const CENTS_TO_MICROCENTS_F32: f32 = CENTS_TO_MICROCENTS as f32;

/// Representation of pitch classes as an integer number of microcents.
/// Avoids the complexity of floating point number comparison, ordering, precision, etc.
#[derive(PartialEq, PartialOrd, Eq, Ord, Copy, Clone, Debug)]
pub struct PitchClass(u32);

impl Display for PitchClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.6}", self.to_cents_f32())
    }
}

impl PitchClass {
    pub const fn from_microcents(microcents: u32) -> Self {
        PitchClass(microcents % OCTAVE_MICROCENTS)
    }

    pub fn distance_to(self, other: PitchClass) -> PitchClassDistance {
        PitchClassDistance(std::cmp::min((self - other).0, (other - self).0))
    }

    pub fn from_midi_note(note: u8) -> Self {
        PitchClass(u32::from(note % 12) * MIDI_NOTE_TO_CENTS * CENTS_TO_MICROCENTS)
    }

    pub fn from_cents_f32(cents: f32) -> Self {
        PitchClass((cents.rem_euclid(1200.0) * CENTS_TO_MICROCENTS_F32).round() as u32)
    }

    pub fn from_midi_note_offset_f32(midi_note_offset_f32: f32) -> Self {
        PitchClass(
            (midi_note_offset_f32.rem_euclid(12.0)
                * MIDI_NOTE_TO_CENTS_F32
                * CENTS_TO_MICROCENTS_F32)
                .round() as u32,
        )
    }

    pub fn to_cents_f32(self) -> f32 {
        self.0 as f32 / CENTS_TO_MICROCENTS_F32
    }

    pub fn with_midi_tuning_offset(self, offset: f32) -> Self {
        nih_dbg!(offset);
        nih_dbg!(
            self.0
                + ((offset.rem_euclid(12.0) * MIDI_NOTE_TO_CENTS_F32 * CENTS_TO_MICROCENTS_F32)
                    .round()) as u32
        );
        PitchClass(
            (self.0
                + ((offset.rem_euclid(12.0) * MIDI_NOTE_TO_CENTS_F32 * CENTS_TO_MICROCENTS_F32)
                    .round()) as u32)
                % OCTAVE_MICROCENTS,
        )
    }

    fn multiply(self, rhs: i32) -> PitchClass {
        if rhs >= 0 {
            PitchClass(((rhs as u64 * u64::from(self.0)) % u64::from(OCTAVE_MICROCENTS)) as u32)
        } else {
            PitchClass(((-rhs as u64 * u64::from((-self).0)) % u64::from(OCTAVE_MICROCENTS)) as u32)
        }
    }
}

impl Add<PitchClass> for PitchClass {
    type Output = PitchClass;
    fn add(self, rhs: PitchClass) -> PitchClass {
        PitchClass((self.0 + rhs.0) % OCTAVE_MICROCENTS)
    }
}

impl Neg for PitchClass {
    type Output = Self;
    fn neg(self) -> PitchClass {
        PitchClass(OCTAVE_MICROCENTS - self.0)
    }
}

impl Sub<PitchClass> for PitchClass {
    type Output = Self;
    fn sub(self, other: PitchClass) -> PitchClass {
        self + -other
    }
}

#[derive(PartialEq, PartialOrd, Eq, Ord, Copy, Clone, Debug)]
pub struct PitchClassDistance(u32);

/// Represents a distance between pitch classes - a most half an octave.
/// Range of [0, 600] cents.
impl PitchClassDistance {
    pub const fn from_microcents(microcents: u32) -> PitchClassDistance {
        // Can't use cmp or min if we want this to be a const fn
        let flipped_microcents = OCTAVE_MICROCENTS - microcents;
        PitchClassDistance(if microcents < flipped_microcents {
            microcents
        } else {
            flipped_microcents
        })
    }
    pub const fn from_cents(cents: u32) -> PitchClassDistance {
        Self::from_microcents(cents * CENTS_TO_MICROCENTS)
    }
}

impl Display for PitchClassDistance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "dist {}", self.0)
    }
}

/// Represents an abstract pitch class as its number of prime factors of 3, 5 and 7
/// C = (0, 0, 0)
pub struct PrimeCountVector {
    pub threes: i32,
    pub fives: i32,
    pub sevens: i32,
}

impl PrimeCountVector {
    pub fn new(threes: i32, fives: i32, sevens: i32) -> PrimeCountVector {
        PrimeCountVector {
            threes,
            fives,
            sevens,
        }
    }

    // Cents value of a pitch class, given tunings for 3, 5 and 7
    pub fn pitch_class(&self, three_cents: f32, five_cents: f32, seven_cents: f32) -> PitchClass {
        PitchClass::from_cents_f32(three_cents).multiply(self.threes)
            + PitchClass::from_cents_f32(five_cents).multiply(self.fives)
            + PitchClass::from_cents_f32(seven_cents).multiply(self.sevens)
    }

    pub fn note_name_info(&self) -> NoteNameInfo {
        static NOTE_NAMES: [char; 7] = ['F', 'C', 'G', 'D', 'A', 'E', 'B'];
        let letter_names_idx = 1 + self.threes + self.fives * 4 + self.sevens * 10;
        NoteNameInfo {
            letter_name: NOTE_NAMES[letter_names_idx.rem_euclid(7) as usize],
            sharps_or_flats: letter_names_idx.div_euclid(7),
            syntonic_commas: -self.fives,
            septimal_commas: -self.sevens,
        }
    }
}

/// Contains information for computing a note's display name
pub struct NoteNameInfo {
    /// Letter name - F, C, G, D, A, E, or B
    pub letter_name: char,

    /// Number of pythagorean semitones (seven fifths, or 2187/2048) sharp or flat
    /// Positive numbers are sharp, negative are flat
    pub sharps_or_flats: i32,

    /// Number of syntonic commas (81/80) added or subtracted
    pub syntonic_commas: i32,

    /// Number of septimal commas (64/63) added or subtracted
    pub septimal_commas: i32,
}

impl NoteNameInfo {
    /// Returns a string for displaying the number of syntonic commas
    /// 1 comma -> +
    /// 2 commas -> ++
    /// 3 commas -> +3
    /// -2 commas -> -2
    pub fn syntonic_comma_str(&self) -> String {
        comma_str(self.syntonic_commas, '+', '-')
    }

    /// Returns a string for displaying the number of sharps/flats
    /// 1 sharp -> #
    /// 2 sharps -> ##
    /// 3 sharps -> #3
    /// 6 flats -> b6
    pub fn sharps_or_flats_str(&self) -> String {
        comma_str(self.sharps_or_flats, '#', 'b')
    }
}

/// Generic way to make a string representing the number of a comma added or subtracted
fn comma_str(comma_count: i32, pos_char: char, neg_char: char) -> String {
    let mut result: String = String::with_capacity(3);
    if comma_count >= 1 {
        result.push(pos_char);
        if comma_count != 1 {
            if comma_count == 2 {
                result.push(pos_char);
            } else if comma_count < 100 {
                result.push_str(&comma_count.to_string());
            } else {
                result.push('?');
            }
        }
    } else if comma_count <= -1 {
        result.push(neg_char);
        if comma_count != -1 {
            if comma_count == -2 {
                result.push(neg_char);
            } else if comma_count > -100 {
                result.push_str(&comma_count.abs().to_string());
            } else {
                result.push('?');
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distance() {
        // Basic case
        assert_eq!(
            PitchClass::from_midi_note(1).distance_to(PitchClass::from_midi_note(10)),
            PitchClassDistance::from_cents(900)
        );

        // Commutative
        assert_eq!(
            PitchClass::from_midi_note(1).distance_to(PitchClass::from_midi_note(10)),
            PitchClass::from_midi_note(10).distance_to(PitchClass::from_midi_note(1))
        );
        // Returns smallest possible distance
        assert_eq!(
            PitchClass::from_microcents(100_000_000)
                .distance_to(PitchClass::from_microcents(900_000_000)),
            PitchClassDistance::from_microcents(400_000_000)
        );
    }

    #[test]
    fn test_multiply() {
        // Basic case
        assert_eq!(
            PitchClass::from_microcents(100_000_000).multiply(9),
            PitchClass::from_microcents(900_000_000)
        );

        // Wraps around
        assert_eq!(
            PitchClass::from_microcents(700_000_000).multiply(4),
            PitchClass::from_microcents(400_000_000)
        );

        // Wraps around (negative)
        assert_eq!(
            PitchClass::from_microcents(700_000_000).multiply(-3),
            PitchClass::from_microcents(300_000_000)
        );

        // Large multiplications are OK
        assert_eq!(
            PitchClass::from_microcents(1_199_999_999).multiply(1_000_000_000),
            PitchClass::from_microcents(200_000_000)
        );

        // Large negative multiplications are OK
        assert_eq!(
            PitchClass::from_microcents(1_199_999_999).multiply(-1_000_000_000),
            PitchClass::from_microcents(1_000_000_000)
        );
    }
}
