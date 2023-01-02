# vidrs: a playground for exploring media capture, processing and publishing with rust

## How to use

On a MacOS system with an attached camera you can call `cargo run` and go to the mentioned server address http://localhost:8080.

## Features

* bindings for camera capturing on MacOS using [madsmtm/objc2](https://github.com/madsmtm/objc2)
* encoding of 4:2:0 camera frames into VP8 frames using [astraw/env-libvpx-sys](https://github.com/astraw/env-libvpx-sys)
* sending VP8 frames via WebRTC to a browser test app using [webrtc-rs/webrtc](https://github.com/webrtc-rs/webrtc)
* handling WebRTC offer/answer exchange with [tokio](https://github.com/tokio-rs/tokio), [hyper](https://github.com/hyperium/hyper) and [serde](https://github.com/serde-rs/serde)

## Ideas for feature work

* receive VP8 video via WebRTC, decode and write or display it
* add support for Linux and Windows camera capturing using [raymanfx/eye-rs](https://github.com/raymanfx/eye-rs) and [l1npengtul/nokhwa](https://github.com/l1npengtul/nokhwa)
* add camera device selection
* better way of figuring out which frame pixel format is preferred for the use case
* use some GUI to show video too, like [egui](https://github.com/emilk/egui), [iced](https://github.com/iced-rs/iced) or [tauri](https://github.com/tauri-apps/tauri)
* use [algesten/str0m](https://github.com/algesten/str0m) for handling WebRTC
* spin off some crates
* some dependencies use `objc` instead of `objc2`, so MacOS bindings could be implemented in different binding ecosystems
