## Notes

* As `dwt-systick-monotonic` depends on `fugit` 0.3.3, you need at least
  rust 1.57 to compile
* nrf-softdevice* requires nightly features, which are enabled via
  `rust-toolchain.toml`. Make sure to run `rustup update` inside this folder.
* Good rtic examples: https://github.com/mciantyre/teensy4-rs/blob/master/examples/rtic_blink.rs