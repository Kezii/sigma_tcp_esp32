# SIGMA_TCP Rust implementation for ESP32

Rust implementation for Analog Devices SigmaStudio's TCP protocol.

The TCP protocol wraps i2c communication to allow for remote control of SigmaDSP devices like the ADAU1701 or ADAU1452.

You can use this software to program and control SigmaDSP devices without the USBi programmer, you need to implement the actual I2C read and write for your platform as a protocol backend, then just use the TCPIPADAU1452 block instead of the USBi block as a communication channel in SigmaStudio.

An ESP32 implementation is provided, enabling you to use SigmaStudio over WiFi.

Inspired by https://github.com/aventuri/sigma_tcp
