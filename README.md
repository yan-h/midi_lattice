# MIDI Lattice

<img width="337" alt="Screenshot 2024-03-02 at 15 35 18" src="https://github.com/yan-h/midi_lattice/assets/8416059/45b425c4-228f-4053-a8c3-23beecc04104">

A VST3/CLAP plugin for visualizing the pitch classes of incoming MIDI notes on a [lattice](https://en.wikipedia.org/wiki/Lattice_(music)). Uses the [nih-plug](https://github.com/robbert-vdh/nih-plug) Rust audio plugin framework. Mostly tested with Bitwig Studio on Windows and Mac.

The plugin displays the pitch classes of incoming notes on a 2D grid, where going upward is a perfect fifth (e.g. C to G), and going right is a major third (e.g. C to E). The tuning of perfect fifths and major thirds is configurable, so the plugin effectively supports any approximation of [5-limit tuning](https://en.wikipedia.org/wiki/Five-limit_tuning). I use it to check my tuning when experimenting with different tuning systems.

Harmonic sevenths can optionally be shown on the top-right (+1) and bottom-left (-1) corners of each note on the lattice. So the plugin has some support for approximations of [7-limit tuning](https://en.wikipedia.org/wiki/7-limit_tuning) as well. To avoid visual clutter, these "z-axis" notes are not displayed except when they play.

Key features:
- Displays pitch classes organized by perfect fifths and major thirds, on a 2D lattice
- Limited display for pitch classes organized by harmonic sevenths - only +1 and -1 on the Z-axis, and only when the notes are playing 
    - The "Show Z axis" parameter determines whether the harmonic seventh axis is shown at all:
        - "No": never display the axis for the harmonic seventh
        - "Auto": only display if the harmonic seventh's tuning is NOT equal to two perfect fourths (as it is in 12-TET)
        - "Yes": always display the axis for the harmonic seventh
- Configurable tuning for the perfect fifth, major third, and harmonic seventh.
- Configurable tuning for the reference pitch (C).
- Note coloring by MIDI channel:
    - Notes on channels 1 through 9 are colored with distinct solid colors
    - 10-14 are colored by pitch height (range is configurable in params)
    - 15 is outlined in white with no fill color
    - 16 is ignored
- Automatic detection of the tuning of intervals and the C reference pitch from incoming MIDI. This is toggleable via the "tuning fork" button on the bottom left. I recommend playing a C major triad for this, or a C harmonic seventh tetrad if you want to tune the seventh harmonic as well. Notes played for tuning detection must be held simultaneously, not arpeggiated.
- Rescalable window - press and drag the button on the bottom right.
- Resizable lattice - press and drag the bottom right corner of the lattice.
- Adjustable lattice position in three dimensions - click and drag the lattice, or set in parameters.

## Demos (with sound)
### 12-tone equal temperament

https://github.com/yan-h/midi_lattice/assets/8416059/98162739-965c-456c-858a-1abfbbb7dd26

### 5-limit just intonation

https://github.com/yan-h/midi_lattice/assets/8416059/7b7d17c9-dcd6-4c78-a514-529bd0a614c7

### 7-limit just intonation

https://github.com/yan-h/midi_lattice/assets/8416059/7259b86b-e47b-46f1-915a-3599a202d991

## Building

After installing [Rust](https://rustup.rs/), you can compile Midi Lattice as follows:

```shell
cargo xtask bundle midi_lattice --release
```
