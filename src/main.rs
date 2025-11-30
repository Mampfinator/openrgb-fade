use std::{collections::HashSet, ffi::CString, str::FromStr, time::Duration};

use hidapi::{HidDevice, HidError};
use openrgb2::{Color, Controller, DeviceType, OpenRgbClient, OpenRgbResult};

use crate::key_mappings::KeyMapping;

mod key_mappings;

static BASE_COLOR: Color = Color::new(255, 100, 255);

#[derive(Default, Clone, Copy, Debug)]
enum FadeState {
    #[default]
    Off,
    On(Brightness),
}

impl FadeState {
    pub fn tick(&mut self) {
        match self {
            Self::On(brightness) => {
                if brightness.tick().is_none() {
                    *self = FadeState::Off;
                }
            }
            _ => {}
        }
    }

    pub fn get_brightness(&self) -> u8 {
        match self {
            Self::On(brightness) => brightness.0,
            Self::Off => 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Brightness(u8);

impl Brightness {
    pub fn tick(&mut self) -> Option<()> {
        if self.0 == 0 {
            None
        } else {
            self.0 -= 5;
            Some(())
        }
    }
}

// I barely know what I'm doing! HID reports can probably be more complicated than
// this thing can cover, but for my specific keyboard (Vulkan TKL), this works well enough. :)
struct HidReader<const B: usize = 1024> {
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

struct KeyEvent(Vec<u8>);

impl KeyEvent {
    pub fn is_down(&self) -> bool {
        self.0.len() >= 5 && self.0[4] > 0
    }

    pub fn key_bytes(&self) -> u16 {
        u16::from_ne_bytes([self.0[2], self.0[3]])
    }
}

async fn setup_device(device: &Controller) -> OpenRgbResult<KeyMapping> {
    let mut hid = HidReader::<512>::new_from_path(device.location()).unwrap();

    println!("Press the keys as they light up.");

    let mut seen = HashSet::new();

    let mut get_next_unique_event = move || {
        loop {
            let event = hid.read_blocking().unwrap();
            let key = event.key_bytes();

            if !seen.contains(&key) {
                seen.insert(key);
                return event;
            }
        }
    };

    let mut keys = Vec::with_capacity(device.num_leds());

    for led in device.led_iter() {
        device.turn_off_leds().await?;
        led.set_led(BASE_COLOR).await?;

        let event = get_next_unique_event();

        keys.push(event.key_bytes());
    }

    Ok(KeyMapping::from(keys))
}

async fn wait_for_server() -> OpenRgbClient {
    for i in 0.. {
        if let Ok(client) = OpenRgbClient::connect().await {
            return client;
        } else {
            tokio::time::sleep(Duration::from_millis((250 * i).max(10000))).await;
        }
    }
    unreachable!()
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> OpenRgbResult<()> {
    let client = wait_for_server().await;

    let keyboard_controller = client
        .get_controllers_of_type(DeviceType::Keyboard)
        .await?
        .into_first()?;

    keyboard_controller.init().await?;

    keyboard_controller.turn_off_leds().await?;

    let path = format!(
        "./{}.keymap",
        keyboard_controller.location().split("/").last().unwrap()
    );

    println!("Using keymap file at {}", path);

    let ledmap = match std::fs::read_to_string(&path) {
        Err(_) => {
            let map = setup_device(&keyboard_controller).await?;
            std::fs::write(path, map.as_file_string()).unwrap();
            map
        }
        Ok(file) => KeyMapping::parse_from_file(file).unwrap(),
    };

    let (tx, rx) = std::sync::mpsc::channel::<KeyEvent>();

    let mut device = HidReader::<512>::new_from_path(keyboard_controller.location()).unwrap();

    tokio::spawn(async move {
        loop {
            tx.send(device.read_blocking().unwrap()).unwrap()
        }
    });

    let mut led_states = vec![FadeState::Off; keyboard_controller.num_leds()];

    loop {
        tokio::time::sleep(Duration::from_millis(25)).await;

        let mut cmd = keyboard_controller.cmd();

        for led in keyboard_controller.led_iter() {
            let state = led_states.get_mut(led.id()).unwrap();

            state.tick();

            let brightness = state.get_brightness();

            let new_color = if brightness == 0 {
                Color::new(0, 0, 0)
            } else {
                BASE_COLOR / (255 - brightness)
            };

            cmd.set_led(led.id(), new_color)?;
        }

        for event in rx.try_iter() {
            if event.is_down()
                && let Some(led) = ledmap.get_led(event.key_bytes())
            {
                led_states[led] = FadeState::On(Brightness(255));
            }
        }

        cmd.execute().await?;
    }
}
