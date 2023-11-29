use crate::midi::DisplayNoteEvent;
use crate::midi::{Voice, VoiceKey};
use heapless::FnvIndexMap;
use midi::update_voices;
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

type Voices = FnvIndexMap<VoiceKey, Voice, 256>;

// This is a shortened version of the gain example with most comments removed, check out
// https://github.com/robbert-vdh/nih-plug/blob/master/plugins/examples/gain/src/lib.rs to get
// started
struct MidiLattice {
    params: Arc<MidiLatticeParams>,

    voices: Voices,
    voices_input: Input<Voices>,
    voices_output: Arc<Mutex<Output<Voices>>>,
}

#[derive(Params)]
pub struct MidiLatticeParams {
    /// The parameter's ID is used to identify the parameter in the wrapped plugin API. As long as
    /// these IDs remain constant, you can rename and reorder these fields as you wish. The
    /// parameters are exposed to the host in the same order they were defined.

    #[persist = "editor-state"]
    pub editor_state: Arc<ViziaState>,

    #[nested(group = "lattice")]
    pub grid_params: Arc<GridParams>,

    #[nested(group = "tuning")]
    pub tuning_params: Arc<TuningParams>,
}

#[derive(Params)]
pub struct GridParams {
    #[persist = "grid-width"]
    pub width: Arc<AtomicU8>,

    #[persist = "grid-height"]
    pub height: Arc<AtomicU8>,

    #[id = "grid-x"]
    pub x: FloatParam,

    #[id = "grid-y"]
    pub y: FloatParam,
}

pub const GRID_MAX_DISTANCE: f32 = 100.0;

impl Default for GridParams {
    fn default() -> Self {
        Self {
            width: Arc::new(AtomicU8::new(7)),
            height: Arc::new(AtomicU8::new(7)),
            x: FloatParam::new(
                "Lattice X",
                0.0,
                FloatRange::Linear {
                    min: -GRID_MAX_DISTANCE,
                    max: GRID_MAX_DISTANCE,
                },
            ),
            y: FloatParam::new(
                "Lattice Y",
                0.0,
                FloatRange::Linear {
                    min: -GRID_MAX_DISTANCE,
                    max: GRID_MAX_DISTANCE,
                },
            ),
        }
    }
}

/// Tuning information for each prime number, in cents
#[derive(Params)]
pub struct TuningParams {
    #[id = "tuning-three"]
    three: FloatParam,
    #[id = "tuning-five"]
    five: FloatParam,
    #[id = "tuning-seven"]
    seven: FloatParam,
}

impl Default for TuningParams {
    fn default() -> Self {
        Self {
            three: FloatParam::new(
                "Perfect Fifth",
                THREE_12TET,
                FloatRange::Linear {
                    min: MIN_THREE,
                    max: MAX_THREE,
                },
            ),
            five: FloatParam::new(
                "Major Third",
                FIVE_12TET,
                FloatRange::Linear {
                    min: MIN_FIVE,
                    max: MAX_FIVE,
                },
            ),
            seven: FloatParam::new(
                "Harmonic Seventh",
                SEVEN_12TET,
                FloatRange::Linear {
                    min: MIN_SEVEN,
                    max: MAX_SEVEN,
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
    const NAME: &'static str = "Midi Lattice";
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
        let start_time = Instant::now();

        let mut event_counter = 0;

        while let Some(event) = context.next_event() {
            update_voices(&mut self.voices, event);

            nih_log!("event: {}", DisplayNoteEvent(event));
            context.send_event(event);

            event_counter += 1;
        }
        //nih_dbg!(&self.voices.clone());

        if event_counter > 0 {
            self.voices_input.write(self.voices.clone());

            for v in self.voices.values() {
                nih_log!("--- voice: {}", v);
            }

            nih_log!(
                "*** process() finished in {} us with {} events",
                start_time.elapsed().as_micros(),
                event_counter
            );
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
        nih_log!("plugin initialized");
        true
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        nih_log!("editor() called");
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

    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Analyzer,
        ClapFeature::Utility,
    ];
}

impl Vst3Plugin for MidiLattice {
    const VST3_CLASS_ID: [u8; 16] = *b"midi_lattice0000";

    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Instrument,
        Vst3SubCategory::Analyzer,
        Vst3SubCategory::Tools,
    ];
}

nih_export_clap!(MidiLattice);
nih_export_vst3!(MidiLattice);
