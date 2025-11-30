use openrgb2::{Color, Controller, OpenRgbResult};

use crate::LedFunction;

#[derive(Default, Clone, Copy, Debug)]
pub enum FadeState {
    #[default]
    Off,
    On(Brightness),
}

impl FadeState {
    pub fn update(&mut self) {
        if let Self::On(brightness) = self
            && brightness.tick().is_none()
        {
            *self = FadeState::Off;
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
pub struct Brightness(u8);

impl Brightness {
    pub const MAX: Brightness = Brightness(255);

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
                self.state[led] = FadeState::On(Brightness::MAX)
            }
        }

        let color = config.color();
        let mut cmd = controller.cmd();

        for led in controller.led_iter() {
            let state = self.state.get_mut(led.id()).unwrap();
            state.update();

            let brightness = state.get_brightness();

            let new_color = if brightness == 0 {
                Color::new(0, 0, 0)
            } else {
                color / (255 - brightness)
            };

            cmd.set_led(led.id(), new_color)?;
        }

        futures_lite::future::block_on(cmd.execute())?;

        Ok(())
    }
}
