pub struct KeyMapping(Vec<u16>);

impl KeyMapping {
    pub fn parse_from_file(file_contents: String) -> Option<Self> {
        file_contents
            .split("\n")
            .map(|line| line.parse())
            .collect::<Result<Vec<u16>, _>>()
            .ok()
            .map(Self::from)
    }

    pub fn as_file_string(&self) -> String {
        self.0
            .iter()
            .copied()
            .map(|key| format!("{}", key))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn get_led(&self, key_byte: u16) -> Option<usize> {
        self.0
            .iter()
            .enumerate()
            .find_map(|(idx, key)| if *key == key_byte { Some(idx) } else { None })
    }
}

impl From<Vec<u16>> for KeyMapping {
    fn from(value: Vec<u16>) -> Self {
        Self(value)
    }
}
