use std::ops::Div;

use nih_plug::nih_log;

use crate::TuningParams;

pub const THREE_JUST: f32 = 7.0195500;
pub const FIVE_JUST: f32 = 3.863137;
pub const SEVEN_JUST: f32 = 9.6882590;

// Two cent values are considered equal if their difference is less than this
pub const CENTS_EPSILON: f32 = 0.001;

/// Representation of a pitch class, in terms of how many factors of 3, 5, and 7 it has
/// C = (0, 0, 0)
pub struct PrimeCountVector {
    pub threes: i32,
    pub fives: i32,
    pub sevens: i32,
}

impl PrimeCountVector {
    pub fn cents(&self, three_cents: f32, five_cents: f32, seven_cents: f32) -> f32 {
        (self.threes as f32 * three_cents
            + self.fives as f32 * five_cents
            + self.sevens as f32 * seven_cents)
            .rem_euclid(1200.0)
            .abs()
    }
}

static NOTE_NAMES: [char; 7] = ['F', 'C', 'G', 'D', 'A', 'E', 'B'];
impl PrimeCountVector {
    pub fn new(threes: i32, fives: i32, sevens: i32) -> PrimeCountVector {
        PrimeCountVector {
            threes,
            fives,
            sevens,
        }
    }
    pub fn note_name_info(&self) -> NoteNameInfo {
        let letter_names_idx = 1 + self.threes + self.fives * 4 + self.sevens * 10;
        NoteNameInfo {
            letter_name: NOTE_NAMES[letter_names_idx.rem_euclid(7) as usize],
            sharps_or_flats: letter_names_idx.div_euclid(7),
            syntonic_commas: -self.fives,
            septimal_commas: -self.sevens,
        }
    }
}

/// Contains information for naming a note
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
