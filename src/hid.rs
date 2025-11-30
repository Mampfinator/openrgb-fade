use std::{ffi::CString, str::FromStr};

use hidapi::{HidDevice, HidError};

// I barely know what I'm doing! HID reports can probably be more complicated than
// this thing can cover, but for my specific keyboard (Vulkan TKL), this works well enough. :)
pub struct HidReader<const B: usize = 1024> {
    buffer: [u8; B],
    device: HidDevice,
}

impl<const B: usize> HidReader<B> {
    pub fn new_from_path(path: &str) -> Option<HidReader<B>> {
        // OpenRGB (for this keyboard at least) prefixes the path with HID because.... yes? So, yeet.
        let path = path.replace("HID: ", "");

        let device = hidapi::HidApi::new()
            .unwrap()
            .open_path(&CString::from_str(&path).ok()?)
            .unwrap();

        Some(Self {
            buffer: [0; B],
            device,
        })
    }

    pub fn read_blocking(&mut self) -> Result<KeyEvent, HidError> {
        let size = self.device.read_timeout(&mut self.buffer, -1)?;
        let slice = &self.buffer[0..size];
        Ok(KeyEvent(Vec::from(slice)))
    }
}

pub struct KeyEvent(Vec<u8>);

impl KeyEvent {
    pub fn is_down(&self) -> bool {
        self.0.len() >= 5 && self.0[4] > 0
    }

    pub fn key_bytes(&self) -> u16 {
        u16::from_ne_bytes([self.0[2], self.0[3]])
    }
}
