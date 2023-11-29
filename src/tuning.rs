pub const THREE_JUST: f32 = 701.95500;
pub const FIVE_JUST: f32 = 386.3137;
pub const SEVEN_JUST: f32 = 968.82590;

pub const THREE_12TET: f32 = 700.0;
pub const FIVE_12TET: f32 = 400.0;
pub const SEVEN_12TET: f32 = 1000.0;

pub static MIN_THREE: f32 = THREE_JUST - TUNING_RANGE_OFFSET;
pub static MAX_THREE: f32 = THREE_JUST + TUNING_RANGE_OFFSET;
pub static MIN_FIVE: f32 = FIVE_JUST - TUNING_RANGE_OFFSET;
pub static MAX_FIVE: f32 = FIVE_JUST + TUNING_RANGE_OFFSET;
pub static MIN_SEVEN: f32 = SEVEN_JUST - TUNING_RANGE_OFFSET;
pub static MAX_SEVEN: f32 = SEVEN_JUST + TUNING_RANGE_OFFSET;

pub const TUNING_RANGE_OFFSET: f32 = 40.0;

// Two cent values are considered equal if their difference is less than this
pub const CENTS_EPSILON: f32 = 0.001;

/// Pitch classes are considered equal if they are within 1/1000 of a cent.
/// Some tolerance above f32 epsilon is nice to ensure that we ignore drift from
/// multiplying a generator interval.
pub fn pitch_classes_equal(a: f32, b: f32) -> bool {
    (a - b).abs() <= CENTS_EPSILON
}

/// Representation of a pitch class, in terms of how many factors of 3, 5, and 7 it has
/// C = (0, 0, 0)
pub struct PrimeCountVector {
    pub threes: i32,
    pub fives: i32,
    pub sevens: i32,
}

impl PrimeCountVector {
    // Cents value of a pitch class, given tunings for 3, 5 and 7
    pub fn cents(&self, three_cents: f32, five_cents: f32, seven_cents: f32) -> f32 {
        // Convert to f64 to avoid loss of precision from multiplying f32 by large numbers
        // Might not matter, but this makes me feel safer
        (self.threes as f64 * three_cents as f64
            + self.fives as f64 * five_cents as f64
            + self.sevens as f64 * seven_cents as f64)
            .rem_euclid(1200.0)
            .abs() as f32
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
