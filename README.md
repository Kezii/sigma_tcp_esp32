# SIGMA_TCP Rust implementation for ESP32

Rust implementation for Analog Devices SigmaStudio's TCP protocol.

The TCP protocol wraps i2c/spi communication to allow for remote control of SigmaDSP devices like the ADAU1701 or ADAU1452.

You can use this software to program and control SigmaDSP devices without the USBi programmer, you need to implement the actual I2C read and write for your platform as a protocol backend, then just use the TCPIPADAU1452 block instead of the USBi block as a communication channel in SigmaStudio.

An ESP32 implementation is provided, enabling you to use SigmaStudio over WiFi.

# Usage with ESP32

1. Clone the repository
2. Install the ESP32 Rust toolchain, follow everything in the official book: https://docs.esp-rs.org/book/installation/index.html
3. Check the I2C pins in the `src/main.rs` file and change them if needed
4. Flash the firmware to the ESP32 using `cargo run --release`
5. Connect the ESP32 to the SigmaDSP device using I2C
6. Connect to the ESP32's WiFi access point (SSID: `ESP32_SIGMADSP`, Password: `123456789`)
7. The TCPIPADAU145x block in SigmaStudio should be configured with the IP address that you see in the serial monitor (should be `192.168.71.1`)
8. Flash and monitor your DSP code from SigmaStudio

Inspired by https://github.com/aventuri/sigma_tcp
