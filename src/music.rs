use super::error::{Error, Kind};
use super::Result;

#[derive(Debug)]
pub enum Note {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
}

impl Note {
    pub fn parse(letter: &str) -> Result<Note> {
        let letter = letter.trim().to_ascii_lowercase();
        match letter.as_str() {
            "a" => Ok(Note::A),
            "b" => Ok(Note::B),
            "c" => Ok(Note::C),
            "d" => Ok(Note::D),
            "e" => Ok(Note::E),
            "f" => Ok(Note::F),
            "g" => Ok(Note::G),
            _ => Err(Error::new("Failed to parse note", Kind::Zinnia)),
        }
    }

    pub fn freq(&self, base_freq: f32) -> f32 {
        match self {
            Note::C => base_freq,
            Note::D => base_freq * 1.125,
            Note::E => base_freq * 1.25,
            Note::F => base_freq * 1.333,
            Note::G => base_freq * 1.5,
            Note::A => base_freq * 1.666,
            Note::B => base_freq * 1.875,
        }
    }
}
