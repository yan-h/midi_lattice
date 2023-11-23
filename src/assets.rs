use nih_plug_vizia::vizia::prelude::Context;

pub const QUICKSAND: &str = "Quicksand";

pub const QUICKSAND_LIGHT: &[u8] = include_bytes!("../assets/quicksand/Quicksand-Light.ttf");
pub const QUICKSAND_REGULAR: &[u8] = include_bytes!("../assets/quicksand/Quicksand-Regular.ttf");
pub const QUICKSAND_MEDIUM: &[u8] = include_bytes!("../assets/quicksand/Quicksand-Medium.ttf");
pub const QUICKSAND_SEMIBOLD: &[u8] = include_bytes!("../assets/quicksand/Quicksand-SemiBold.ttf");
pub const QUICKSAND_BOLD: &[u8] = include_bytes!("../assets/quicksand/Quicksand-Bold.ttf");

pub const JET_BRAINS_MONO_REGULAR: &[u8] =
    include_bytes!("../assets/jet_brains_mono/JetBrainsMono-Regular.ttf");

pub fn register_quicksand(cx: &mut Context) {
    cx.add_fonts_mem(&[QUICKSAND_LIGHT]);
    cx.add_fonts_mem(&[QUICKSAND_REGULAR]);
    cx.add_fonts_mem(&[QUICKSAND_MEDIUM]);
    cx.add_fonts_mem(&[QUICKSAND_SEMIBOLD]);
    cx.add_fonts_mem(&[QUICKSAND_BOLD]);
}
