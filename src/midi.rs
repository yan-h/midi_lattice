use core::hash::{Hash, Hasher};
use hash32;
use hash32_derive::Hash32;
use nih_plug::midi::NoteEvent;
use nih_plug::{nih_error, nih_log};

use std::fmt;
use std::fmt::Display;

use crate::tuning::PitchClass;
use crate::Voices;

#[derive(Debug, PartialEq, Clone, Copy, PartialOrd)]
pub struct MidiVoice {
    voice_id: Option<i32>,
    channel: u8,
    note: u8,
    pitch: f32,
    pitch_class: PitchClass,
}

impl Hash for MidiVoice {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.note.hash(state);
        self.channel.hash(state);
    }
}

impl hash32::Hash for MidiVoice {
    fn hash<H>(&self, state: &mut H)
    where
        H: hash32::Hasher,
    {
        hash32::Hash::hash(&self.note, state);
        hash32::Hash::hash(&self.channel, state);
    }
}

impl MidiVoice {
    pub fn new(
        voice_id: Option<i32>,
        channel: u8,
        note: u8,
        pitch: f32,
        pitch_class: PitchClass,
    ) -> Self {
        MidiVoice {
            voice_id,
            channel,
            note,
            pitch,
            pitch_class,
        }
    }

    pub fn from_midi_data(voice_id: Option<i32>, channel: u8, note: u8) -> Self {
        Self::new(
            voice_id,
            channel,
            note,
            note as f32,
            PitchClass::from_midi_note(note),
        )
    }

    fn set_tuning(&mut self, tuning_offset: f32) {
        self.pitch = self.note as f32 + tuning_offset;
        self.pitch_class = PitchClass::from_midi_note(self.note)
            + PitchClass::from_midi_note_offset_f32(tuning_offset);
    }

    pub fn get_pitch(&self) -> f32 {
        self.pitch
    }

    pub fn get_pitch_class(&self) -> PitchClass {
        self.pitch_class
    }

    pub fn get_channel(&self) -> u8 {
        self.channel
    }
}

impl Display for MidiVoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{ note {}, ch {}, pitch {}, pitch class {} }}",
            self.note, self.channel, self.pitch, self.pitch_class
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
                "{{ tune: note {}, ch {}, id {:?}, tun {:.9} }}",
                note, channel, voice_id, tuning
            ),
            DisplayNoteEvent(note_event) => {
                write!(f, "other event: {:?}", note_event)
            }
        }
    }
}

pub fn update_midi_voices(voices: &mut Voices, event: NoteEvent<()>) {
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
                MidiVoice::from_midi_data(voice_id, channel, note),
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
            let cur_voice: Option<&mut MidiVoice> = voices.get_mut(&VoiceKey { channel, note });
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
