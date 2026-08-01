use openrgb2::{Color, Controller, OpenRgbResult};

use crate::{LedFunction, config::Config};

#[derive(Default, Clone, Copy, Debug)]
pub enum FadeState {
    #[default]
    Off,
    On {
        brightness: Brightness,
        elapsed_ticks: usize,
    },
}

impl FadeState {
    pub fn update(&mut self, config: &Config) {
        if let Self::On {
            brightness,
            elapsed_ticks,
        } = self
        {
            *elapsed_ticks += 1;
            if *elapsed_ticks >= config.fadeout_delay() && brightness.tick().is_none() {
                *self = FadeState::Off;
            }
        }
    }

    pub fn get_brightness(&self) -> u8 {
        match self {
            Self::On { brightness, .. } => brightness.0,
            Self::Off => 0,
        }
    }

    pub fn on(brightness: u8) -> Self {
        Self::On {
            brightness: Brightness::new(brightness),
            elapsed_ticks: 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Brightness(u8);

impl Brightness {
    pub fn new(brightness: u8) -> Self {
        Self(brightness)
    }

    pub fn tick(&mut self) -> Option<()> {
        if self.0 == 0 {
            None
        } else {
            self.0 -= 1;
            Some(())
        }
    }
}

pub struct FadeLeds {
    state: Vec<FadeState>,
}

impl LedFunction for FadeLeds {
    fn new(controller: &Controller) -> Self {
        Self {
            state: vec![FadeState::Off; controller.num_leds()],
        }
    }

    fn update(
        &mut self,
        config: &crate::config::Config,
        events: &[crate::hid::KeyEvent],
        key_map: &crate::key_mappings::KeyMapping,
        controller: &Controller,
    ) -> OpenRgbResult<()> {
        for event in events.iter() {
            if event.is_down()
                && let Some(led) = key_map.get_led(event.key_bytes())
            {
                self.state[led] = FadeState::on(config.max_brightness());
            }
        }

        let color = config.color();
        let mut cmd = controller.cmd();

        for led in controller.led_iter() {
            let state = self.state.get_mut(led.id()).unwrap();
            state.update(&config);

            let brightness = state.get_brightness();

            let new_color = if brightness <= config.brightness_cutoff() {
                Color::new(0, 0, 0)
            } else {
                color / (255 - brightness).max(1)
            };

            cmd.set_led(led.id(), new_color)?;
        }

        futures_lite::future::block_on(cmd.execute())?;

        Ok(())
    }
}
