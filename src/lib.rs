use crate::midi::{MidiVoice, VoiceKey};
use heapless::FnvIndexMap;
use midi::update_midi_voices;
use nih_plug::prelude::*;
use nih_plug_vizia::ViziaState;
use tuning::*;

use std::sync::atomic::AtomicU8;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use triple_buffer::{Input, Output, TripleBuffer};

mod assets;
mod editor;
mod midi;
mod tuning;

type Voices = FnvIndexMap<VoiceKey, MidiVoice, 256>;

struct MidiLattice {
    params: Arc<MidiLatticeParams>,

    voices: Voices,
    voices_input: Input<Voices>,
    voices_output: Arc<Mutex<Output<Voices>>>,
}

#[derive(Params)]
pub struct MidiLatticeParams {
    /// Editor state. This plugin mostly cares about it because it stores window size.
    #[persist = "editor-state"]
    pub editor_state: Arc<ViziaState>,

    #[nested(group = "tuning")]
    pub tuning_params: Arc<TuningParams>,

    #[nested(group = "grid")]
    pub grid_params: Arc<GridParams>,
}

#[derive(Params)]
pub struct GridParams {
    /// Width of the grid display, in grid nodes
    #[persist = "grid-width"]
    pub width: Arc<AtomicU8>,

    // Height of the grid display, in grid nodes
    #[persist = "grid-height"]
    pub height: Arc<AtomicU8>,

    // X offset of the grid from the origin, C
    #[id = "grid-x"]
    pub x: FloatParam,

    // Y offset of the grid from the origin, C
    #[id = "grid-y"]
    pub y: FloatParam,

    // Z offset of the grid form the origin, C
    #[id = "grid-z"]
    pub z: IntParam,

    // How many seconds a note remains highlighted after release
    #[id = "highlight-time"]
    pub highlight_time: FloatParam,

    // Whether to show the Z axis (representing the prime factor 7)
    #[id = "display-z-axis"]
    pub show_z_axis: EnumParam<ShowZAxis>,

    // The pitch with the "darkest" color, on channels colored by pitch
    #[id = "darkest-pitch"]
    pub darkest_pitch: FloatParam,

    // The pitch with the "brightest" color, on channels colored by pitch
    #[id = "brightest-pitch"]
    pub brightest_pitch: FloatParam,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Enum)]
pub enum ShowZAxis {
    Yes,
    Auto,
    No,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Enum)]
pub enum NoteColorScheme {
    Channel,
    Pitch,
}

const MAX_GRID_OFFSET: f32 = 20.0;

impl Default for GridParams {
    fn default() -> Self {
        Self {
            width: Arc::new(AtomicU8::new(7)),
            height: Arc::new(AtomicU8::new(7)),
            x: FloatParam::new(
                "Grid X",
                0.0,
                FloatRange::Linear {
                    min: -MAX_GRID_OFFSET,
                    max: MAX_GRID_OFFSET,
                },
            ),
            y: FloatParam::new(
                "Grid Y",
                0.0,
                FloatRange::Linear {
                    min: -MAX_GRID_OFFSET,
                    max: MAX_GRID_OFFSET,
                },
            ),
            z: IntParam::new(
                "Grid Z",
                0,
                IntRange::Linear {
                    min: -MAX_GRID_OFFSET as i32,
                    max: MAX_GRID_OFFSET as i32,
                },
            ),
            highlight_time: FloatParam::new(
                "Note Highlight (sec)",
                1.0,
                FloatRange::Skewed {
                    min: 0.0,
                    max: 100.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            ),
            show_z_axis: EnumParam::new("Show Z Axis", ShowZAxis::Auto),
            darkest_pitch: FloatParam::new(
                "Darkest pitch",
                30.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 60.0,
                },
            ),
            brightest_pitch: FloatParam::new(
                "Brightest pitch",
                90.0,
                FloatRange::Linear {
                    min: 60.0,
                    max: 120.0,
                },
            ),
        }
    }
}

/// Tuning information for each prime harmonic, in cents
#[derive(Params)]
pub struct TuningParams {
    #[id = "tuning-c-offset"]
    c_offset: FloatParam,

    #[id = "tuning-three"]
    three: FloatParam,

    #[id = "tuning-five"]
    five: FloatParam,

    #[id = "tuning-seven"]
    seven: FloatParam,

    #[id = "tuning-tolerance"]
    tolerance: FloatParam,
}

// Range for the tuning parameter for each prime harmonic
const MAX_TUNING_OFFSET: f32 = 40.0;

impl Default for TuningParams {
    fn default() -> Self {
        Self {
            c_offset: FloatParam::new(
                "C Tuning Offset (cents)",
                0.0,
                FloatRange::Linear {
                    min: -600.0,
                    max: 600.0,
                },
            ),
            three: FloatParam::new(
                "Perfect Fifth (cents)",
                THREE_12TET_F32,
                FloatRange::Linear {
                    min: THREE_JUST_F32 - MAX_TUNING_OFFSET,
                    max: THREE_JUST_F32 + MAX_TUNING_OFFSET,
                },
            ),
            five: FloatParam::new(
                "Major Third (cents)",
                FIVE_12TET_F32,
                FloatRange::Linear {
                    min: FIVE_JUST_F32 - MAX_TUNING_OFFSET,
                    max: FIVE_JUST_F32 + MAX_TUNING_OFFSET,
                },
            ),
            seven: FloatParam::new(
                "Harmonic Seventh (cents)",
                SEVEN_12TET_F32,
                FloatRange::Linear {
                    min: SEVEN_JUST_F32 - MAX_TUNING_OFFSET,
                    max: SEVEN_JUST_F32 + MAX_TUNING_OFFSET,
                },
            ),
            tolerance: FloatParam::new(
                "Tuning Tolerance (cents)",
                0.5,
                FloatRange::Skewed {
                    min: 0.001,
                    max: 49.999,
                    factor: FloatRange::skew_factor(-2.5),
                },
            ),
        }
    }
}

impl MidiLatticeParams {
    fn new(grid_params: Arc<GridParams>) -> Self {
        nih_log!("created default params");
        Self {
            editor_state: editor::vizia_state(grid_params.clone()),
            grid_params: grid_params,
            tuning_params: Arc::new(TuningParams::default()),
        }
    }
}

impl Default for MidiLattice {
    fn default() -> Self {
        nih_log!("default");
        let (input, output) = TripleBuffer::default().split();
        Self {
            params: Arc::new(MidiLatticeParams::new(Arc::default())),
            voices: FnvIndexMap::new(),
            voices_input: input,
            voices_output: Arc::new(Mutex::new(output)),
        }
    }
}

impl Plugin for MidiLattice {
    const NAME: &'static str = "MIDI Lattice";
    const VENDOR: &'static str = "Yan Han";
    const URL: &'static str = env!("CARGO_PKG_HOMEPAGE");
    const EMAIL: &'static str = "yanhan13@gmail.com";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    // The first audio IO layout is used as the default. The other layouts may be selected either
    // explicitly or automatically by the host or the user depending on the plugin API/backend.
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),

        aux_input_ports: &[],
        aux_output_ports: &[],

        // Individual ports and the layout as a whole can be named here. By default these names
        // are generated as needed. This layout will be called 'Stereo', while a layout with
        // only one input and output channel would be called 'Mono'.
        names: PortNames::const_default(),
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::MidiCCs;

    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

    // If the plugin can send or receive SysEx messages, it can define a type to wrap around those
    // messages here. The type implements the `SysExMessage` trait, which allows conversion to and
    // from plain byte buffers.
    type SysExMessage = ();
    // More advanced plugins can use this to run expensive background tasks. See the field's
    // documentation for more information. `()` means that the plugin does not have any background
    // tasks.
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn process(
        &mut self,
        _buffer: &mut Buffer<'_>,
        _aux: &mut AuxiliaryBuffers<'_>,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let _start_time = Instant::now();

        let mut event_counter = 0;

        while let Some(event) = context.next_event() {
            update_midi_voices(&mut self.voices, event);

            //nih_log!("event: {}", DisplayNoteEvent(event));
            context.send_event(event);

            event_counter += 1;
        }

        if event_counter > 0 {
            self.voices_input.write(self.voices.clone());

            for _v in self.voices.values() {
                //nih_log!("--- voice: {}", v);
            }
        }

        ProcessStatus::Normal
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        _buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        // Resize buffers and perform other potentially expensive initialization operations here.
        // The `reset()` function is always called right after this function. You can remove this
        // function if you do not need it.
        true
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(editor::Data::new(
            self.params.clone(),
            self.voices_output.clone(),
        ))
    }
}

impl ClapPlugin for MidiLattice {
    const CLAP_ID: &'static str = "github.com/yan-h/midi_lattice";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Visualizes incoming MIDI in a tuning lattice");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_POLY_MODULATION_CONFIG: Option<PolyModulationConfig> = Some(PolyModulationConfig {
        max_voice_capacity: 256,
        supports_overlapping_voices: true,
    });

    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::NoteEffect,
        ClapFeature::Analyzer,
        ClapFeature::Utility,
    ];
}

impl Vst3Plugin for MidiLattice {
    const VST3_CLASS_ID: [u8; 16] = *b"midi_lattice0000";

    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Fx,
        Vst3SubCategory::Analyzer,
        Vst3SubCategory::Tools,
    ];
}

nih_export_clap!(MidiLattice);
nih_export_vst3!(MidiLattice);
