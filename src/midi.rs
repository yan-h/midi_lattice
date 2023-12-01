use core::hash::{Hash, Hasher};
use hash32;
use hash32_derive::Hash32;
use nih_plug::midi::NoteEvent;
use nih_plug::{nih_error, nih_log};

use std::fmt;
use std::fmt::Display;

use crate::tuning::PitchClass;
use crate::Voices;

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct Voice {
    voice_id: Option<i32>,
    channel: u8,
    note: u8,
    // Pitch in semitones
    pitch: f32,
    pitch_class: PitchClass,
}

impl Eq for Voice {}

impl Hash for Voice {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.note.hash(state);
        self.channel.hash(state);
    }
}

impl hash32::Hash for Voice {
    fn hash<H>(&self, state: &mut H)
    where
        H: hash32::Hasher,
    {
        hash32::Hash::hash(&self.note, state);
        hash32::Hash::hash(&self.channel, state);
    }
}

impl Voice {
    fn new(voice_id: Option<i32>, channel: u8, note: u8) -> Self {
        Voice {
            voice_id,
            channel,
            note,
            pitch: note as f32,
            pitch_class: PitchClass::from_midi_note(note),
        }
    }

    fn set_tuning(&mut self, tuning_offset: f32) {
        self.pitch = (self.note as f32) + tuning_offset;
        self.pitch_class = self.pitch_class.with_midi_tuning_offset(tuning_offset);
    }

    pub fn get_pitch_class(&self) -> PitchClass {
        self.pitch_class
    }
}

impl Display for Voice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{ note {}, ch {}, pitch {} }}",
            self.note, self.channel, self.pitch
        )
    }
}

#[derive(PartialEq, Eq, Debug, Hash, Copy, Clone, Hash32)]
pub struct VoiceKey {
    /// The note's channel, in `0..16`.
    pub channel: u8,
    /// The note's MIDI key number, in `0..128`.
    pub note: u8,
}

pub struct DisplayNoteEvent(pub NoteEvent<()>);

impl Display for DisplayNoteEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DisplayNoteEvent(NoteEvent::NoteOn {
                timing: _,
                voice_id,
                channel,
                note,
                velocity,
            }) => write!(
                f,
                "{{ on | note {}, ch {}, id {:?}, vel {} }}",
                note, channel, voice_id, velocity
            ),
            DisplayNoteEvent(NoteEvent::NoteOff {
                timing: _,
                voice_id,
                channel,
                note,
                velocity,
            }) => write!(
                f,
                "{{ off: note {}, ch {}, id {:?}, vel {} }}",
                note, channel, voice_id, velocity
            ),
            DisplayNoteEvent(NoteEvent::PolyTuning {
                timing: _,
                voice_id,
                channel,
                note,
                tuning,
            }) => write!(
                f,
                "{{ tune: note {}, ch {}, id {:?}, tun {} }}",
                note, channel, voice_id, tuning
            ),
            DisplayNoteEvent(note_event) => {
                write!(f, "other event: {:?}", note_event)
            }
        }
    }
}

pub fn update_voices(voices: &mut Voices, event: NoteEvent<()>) {
    match event {
        NoteEvent::NoteOn {
            timing: _,
            voice_id,
            channel,
            note,
            velocity: _,
        } => {
            match voices.insert(
                VoiceKey { note, channel },
                Voice::new(voice_id, channel, note),
            ) {
                Ok(Some(_)) => {
                    nih_error!(
                        "!!! Received note on for existing voice: {}",
                        DisplayNoteEvent(event)
                    );
                }
                Err(_) => {
                    nih_error!("!!! Too many voices")
                }
                _ => {}
            }
        }
        NoteEvent::NoteOff {
            timing: _,
            voice_id: _,
            channel,
            note,
            velocity: _,
        } => match voices.remove(&VoiceKey { note, channel }) {
            None => {
                nih_log!(
                    "!!! Received off for nonexisting voice: {}",
                    DisplayNoteEvent(event)
                );
            }
            _ => {}
        },
        NoteEvent::PolyTuning {
            timing: _,
            voice_id: _,
            channel,
            note,
            tuning,
        } => {
            let cur_voice: Option<&mut Voice> = voices.get_mut(&VoiceKey { channel, note });
            match cur_voice {
                None => {
                    nih_log!(
                        "!!! Received tuning for nonexistent voice: {}",
                        DisplayNoteEvent(event)
                    );
                }
                Some(voice) => {
                    voice.set_tuning(tuning);
                }
            }
        }
        _ => {}
    }
}
