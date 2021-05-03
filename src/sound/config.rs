use std::slice::Iter;

pub struct SoundConfig {
    freq: f32,
    phase: f32,
    amplitude_scale: f32,
}

impl SoundConfig {
    pub fn new(freq: f32, phase: f32, amplitude_scale: f32) -> Self {
        SoundConfig {
            freq,
            phase,
            amplitude_scale,
        }
    }
}

pub struct SoundConfigCollection {
    configs: Option<Vec<SoundConfig>>,
}

impl SoundConfigCollection {
    pub fn new() -> Self {
        SoundConfigCollection { configs: None }
    }

    pub fn with_configs(configs: &[(f32, f32, f32)]) -> Self {
        let configs: Vec<SoundConfig> = configs
            .iter()
            .map(|c| SoundConfig::new(c.0, c.1, c.2))
            .collect();

        Self {
            configs: Some(configs),
        }
    }

    pub fn add_config(&mut self, freq: f32, phase: f32, amplitude_scale: f32) {
        let config = SoundConfig::new(freq, phase, amplitude_scale);
        match self.configs {
            Some(ref mut configs) => configs.push(config),
            None => self.configs = Some(vec![config]),
        }
    }

    pub fn iter<'a>(&'a self) -> SoundConfigIterator<'a> {
        SoundConfigIterator {
            iterator: match self.configs {
                Some(ref configs) => Some(configs.iter()),
                None => None,
            },
        }
    }
}

impl<'a> IntoIterator for &'a SoundConfigCollection {
    type Item = &'a SoundConfig;

    type IntoIter = SoundConfigIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub struct SoundConfigIterator<'a> {
    iterator: Option<Iter<'a, SoundConfig>>,
}

impl<'a> SoundConfigIterator<'a> {
    pub fn map_freq<F, T>(self, f: F) -> Box<dyn Iterator<Item = T> + 'a>
    where
        F: Fn(f32) -> T + 'a,
    {
        Box::new(self.iterator.unwrap().map(move |c| (f)(c.freq)))
    }

    pub fn map_phase<F, T>(self, f: F) -> Box<dyn Iterator<Item = T> + 'a>
    where
        F: Fn(f32) -> T + 'a,
    {
        Box::new(self.iterator.unwrap().map(move |c| (f)(c.phase)))
    }

    pub fn map_amplitude<F, T>(self, f: F) -> Box<dyn Iterator<Item = T> + 'a>
    where
        F: Fn(f32) -> T + 'a,
    {
        Box::new(self.iterator.unwrap().map(move |c| (f)(c.amplitude_scale)))
    }
}

impl<'a> Iterator for SoundConfigIterator<'a> {
    type Item = &'a SoundConfig;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iterator {
            Some(ref mut iter) => iter.next(),
            None => None,
        }
    }
}
