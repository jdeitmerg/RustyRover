# RustyRover

This is the firmware for a cheap robot car kit, which had its
[brains](https://docs.arduino.cc/hardware/uno-rev3) replaced by an
[NRF52832 board](https://www.adafruit.com/product/3406). As implied by the
name, the firmware is written in Rust. RustyRover is a hobby project aimed at
learning about

* Embedded Rust
  * Rust and its ecosystem
  * [The specialized tooling](https://probe.rs/)
  * [RTIC](https://rtic.rs/)
* A new chip (the nRF52)
* A new communication stack (BLE)

## Status

The softdevice and RTIC are set up to nicely play together. BLE advertisement
works and the basics of event handling are implemented. Everything is in a
proof-of-concept state.

## Setup

* Install nightly rust, probe-run:

```bash
$ rustup update
$ cargo install probe-run probe-rs-cli
```

* Flash softdevice s112 v7.3.0 (after downloading from nRF website): 

```bash
$ probe-rs-cli erase --chip nrf52832
$ probe-rs-cli download --chip nrf52832 --format hex s112_nrf52_7.3.0_softdevice.hex
```

* Connect to target using debug probe supported by probe-rs.

## Running

```bash
$ export DEFMT_LOG=debug
$ cargo run
```

## Notes

* As `dwt-systick-monotonic` depends on `fugit` 0.3.3, you need at least
  rust 1.57 to compile
* nrf-softdevice* requires nightly features, which are enabled via
  `rust-toolchain.toml`. Make sure to run `rustup update` inside this folder.
* Good rtic examples: https://github.com/mciantyre/teensy4-rs/blob/master/examples/rtic_blink.rs
* The SoftDevice and defmt don't go well together. Using defmt in the
  event notification interrupt makes the SoftDevice assert, possibly because
  it detects some debug-fishiness going on. It definitely doesn't have
  anything to to with blocking the notification interrupt, as busy-looping
  for 500ms is no problem. Rerouting the event to another RTIC task works
  around this issue (see `softdev_event_notify_interrupt` and
  `softdev_event_notify` in [src/main.rs](src/main.rs))
