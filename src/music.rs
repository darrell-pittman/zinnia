use std::{convert::TryFrom, u32};

use super::error::{Error, Kind};
use super::Result;
use std::result::Result as StdResult;

const KEYS_PER_OCTAVE: i32 = 12;
const START_KEY_OFFSET: i32 = 8;

#[derive(Debug, Clone, Copy)]
pub enum Octave {
    Zero = 0,
    One = 1,
    Two = 2,
    Three = 3,
    Four = 4,
    Five = 5,
    Six = 6,
    Seven = 7,
    Eight = 8,
}

impl TryFrom<char> for Octave {
    type Error = Error;

    fn try_from(value: char) -> StdResult<Self, Self::Error> {
        match value {
            '0' => Ok(Octave::Zero),
            '1' => Ok(Octave::One),
            '2' => Ok(Octave::Two),
            '3' => Ok(Octave::Three),
            '4' => Ok(Octave::Four),
            '5' => Ok(Octave::Five),
            '6' => Ok(Octave::Six),
            '7' => Ok(Octave::Seven),
            '8' => Ok(Octave::Eight),
            _ => Err(Error::new("Invalid Octave", Kind::Zinnia)),
        }
    }
}

#[derive(Debug)]
pub enum Note {
    A(Octave),
    ASharp(Octave),
    BFlat(Octave),
    B(Octave),
    C(Octave),
    CSharp(Octave),
    DFlat(Octave),
    D(Octave),
    DSharp(Octave),
    EFlat(Octave),
    E(Octave),
    F(Octave),
    FSharp(Octave),
    GFlat(Octave),
    G(Octave),
    GSharp(Octave),
    AFlat(Octave),
}

impl Note {
    pub fn parse(symbol: &str) -> Result<Note> {
        let symbol = symbol.trim().to_ascii_lowercase();

        match symbol.chars().nth(0) {
            Some(octave) => {
                let octave = TryFrom::<char>::try_from(octave)?;
                match symbol.chars().nth(1) {
                    Some(note) => match note {
                        'a' => Ok(Note::A(octave)),
                        'b' => Ok(Note::B(octave)),
                        'c' => Ok(Note::C(octave)),
                        'd' => Ok(Note::D(octave)),
                        'e' => Ok(Note::E(octave)),
                        'f' => Ok(Note::F(octave)),
                        'g' => Ok(Note::G(octave)),
                        _ => Err(Error::new(
                            "Failed to parse note",
                            Kind::Zinnia,
                        )),
                    },
                    None => {
                        Err(Error::new("Failed to parse note", Kind::Zinnia))
                    }
                }
            }
            None => Err(Error::new("Failed to parse note", Kind::Zinnia)),
        }
    }

    pub fn freq(&self) -> Result<f32> {
        let key_number = self.key_number()?;
        Ok(2.0f32.powf((key_number as f32 - 49.0) / 12.0) * 440.0)
    }

    fn key_number(&self) -> Result<u32> {
        let (key_offset, octave) = match self {
            Note::C(octave) => (0, octave),
            Note::CSharp(octave) | Note::DFlat(octave) => (1, octave),
            Note::D(octave) => (2, octave),
            Note::DSharp(octave) | Note::EFlat(octave) => (3, octave),
            Note::E(octave) => (4, octave),
            Note::F(octave) => (5, octave),
            Note::FSharp(octave) | Note::GFlat(octave) => (6, octave),
            Note::G(octave) => (7, octave),
            Note::GSharp(octave) | Note::AFlat(octave) => (8, octave),
            Note::A(octave) => (9, octave),
            Note::ASharp(octave) | Note::BFlat(octave) => (10, octave),
            Note::B(octave) => (11, octave),
        };

        let key: i32 =
            *octave as i32 * KEYS_PER_OCTAVE + key_offset - START_KEY_OFFSET;

        if key < 1 || key > 88 {
            Err(Error::new("Invalid Note", Kind::Zinnia))
        } else {
            Ok(key as u32)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn c_0_fail() -> Result<()> {
        match Note::C(Octave::Zero).key_number() {
            Err(_) => Ok(()),
            _ => Err(Error::new("Expected an error", Kind::Zinnia)),
        }
    }
    #[test]
    fn gsharp_0_fail() -> Result<()> {
        match Note::GSharp(Octave::Zero).key_number() {
            Err(_) => Ok(()),
            _ => Err(Error::new("Expected an error", Kind::Zinnia)),
        }
    }
    #[test]
    fn a_0_ok() {
        assert_eq!(Note::A(Octave::Zero).key_number().unwrap(), 1);
    }
    #[test]
    fn b_0_ok() {
        assert_eq!(Note::B(Octave::Zero).key_number().unwrap(), 3);
    }
    #[test]
    fn c_1_ok() {
        assert_eq!(Note::C(Octave::One).key_number().unwrap(), 4);
    }
    #[test]
    fn middle_c_ok() {
        assert_eq!(Note::C(Octave::Four).key_number().unwrap(), 40);
    }
    #[test]
    fn a_440_ok() {
        assert_eq!(Note::A(Octave::Four).key_number().unwrap(), 49);
    }
    #[test]
    fn c_8_ok() {
        assert_eq!(Note::C(Octave::Eight).key_number().unwrap(), 88);
    }
    #[test]
    fn csharp_8_fail() -> Result<()> {
        match Note::CSharp(Octave::Eight).key_number() {
            Err(_) => Ok(()),
            _ => Err(Error::new("Expected an error", Kind::Zinnia)),
        }
    }
}
