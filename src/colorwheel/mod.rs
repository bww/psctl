use colored::{Color, Style, Colorize, ColoredString};

/// Wheel is a color wheel instance
pub struct Wheel {
  colors: Vec<Color>,
  style: Style,
}

impl Wheel {
  pub fn default() -> Self {
    Self {
      colors: vec![
        Color::Magenta,
        Color::Blue,
        Color::Green,
        Color::Cyan,
        Color::Yellow,
      ],
      style: Style::default().bold(),
    }
  }

  pub fn colorize(&self, index: usize, msg: String) -> ColoredString {
    let mut c = msg.color(self.colors[index % self.colors.len()]);
    c.style = self.style;
    c
  }
}
