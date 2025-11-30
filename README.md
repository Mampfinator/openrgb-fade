# OpenRGB-Fade
Uses the OpenRGB SDK to provide crude key fade-out for keyboards that don't support reactive effects.

# Requirements
Requires an OpenRGB SDK server running on the same machine with default port (might get around to making this configurable soonTM).

# Setup
With your keyboard connected and the SDK server running, run the program once. This will create a config file in `~/.config/openrgb-fade/config.jsonc`.
It will also start a setup program to map keyboard LEDs to keys (since the SDK doesn't appear to expose that). Just press the keys as they light up and you should be good to go!
This creates a keymap file (also found in `~/.config/openrgb-fade`).
