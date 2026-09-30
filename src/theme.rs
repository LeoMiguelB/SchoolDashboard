use ratatui::style::Color;

#[allow(dead_code)]
pub struct Theme;

#[allow(dead_code)]
impl Theme {
    // Brand / Accents
    pub const LAVENDER: Color = Color::Rgb(180, 190, 254);
    pub const ROSEWATER: Color = Color::Rgb(245, 224, 220);
    pub const MAUVE: Color = Color::Rgb(203, 166, 247);
    pub const PINK: Color = Color::Rgb(245, 194, 231);
    pub const RED: Color = Color::Rgb(243, 139, 168);
    pub const PEACH: Color = Color::Rgb(250, 179, 135);
    pub const YELLOW: Color = Color::Rgb(249, 226, 175);
    pub const GREEN: Color = Color::Rgb(166, 227, 161);
    pub const TEAL: Color = Color::Rgb(148, 226, 213);
    pub const SKY: Color = Color::Rgb(137, 220, 235);
    pub const SAPPHIRE: Color = Color::Rgb(116, 199, 236);
    pub const BLUE: Color = Color::Rgb(137, 180, 250);

    // Text & Surfaces
    pub const TEXT: Color = Color::Rgb(205, 214, 244);
    pub const SUBTEXT: Color = Color::Rgb(166, 173, 200);
    pub const MUTED: Color = Color::Rgb(108, 112, 134);
    pub const OVERLAY: Color = Color::Rgb(88, 91, 112);
    pub const SURFACE_ACTIVE: Color = Color::Rgb(49, 50, 68);
    pub const SURFACE_HOVER: Color = Color::Rgb(69, 71, 90);
    pub const BASE_BG: Color = Color::Rgb(30, 30, 46);

    pub fn course_color(name: &str) -> Color {
        match name.to_lowercase().as_str() {
            "cyan" | "sky" => Self::SKY,
            "magenta" | "mauve" => Self::MAUVE,
            "yellow" | "peach" => Self::YELLOW,
            "green" => Self::GREEN,
            "blue" | "sapphire" => Self::SAPPHIRE,
            "red" => Self::RED,
            _ => Self::LAVENDER,
        }
    }
}
