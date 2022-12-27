#![allow(unused)]

//! Rust interface to libvpx encoder
//!
//! This crate provides a Rust API to use
//! [libvpx](https://en.wikipedia.org/wiki/Libvpx) for encoding images.
//!
//! It it based entirely on code from [srs](https://crates.io/crates/srs).
//! Compared to the original `srs`, this code has been simplified for use as a
//! library and updated to add support for both the VP8 codec and (optionally)
//! the VP9 codec.
//!
//! # Optional features
//!
//! Compile with the cargo feature `vp9` to enable support for the VP9 codec.
//!
//! # Example
//!
//! An example of using `vpx-encode` can be found in the [`record-screen`]()
//! program. The source code for `record-screen` is in the [vpx-encode git
//! repository]().
//!
//! # Contributing
//!
//! All contributions are appreciated.

// vpx_sys is provided by the `env-libvpx-sys` crate

#![cfg_attr(
    feature = "backtrace",
    feature(error_generic_member_access, provide_any)
)]

use std::{
    mem::MaybeUninit,
    os::raw::{c_int, c_uint, c_ulong},
};

#[cfg(feature = "backtrace")]
use std::backtrace::Backtrace;
use std::{ptr, slice};

use thiserror::Error;

#[cfg(feature = "vp9")]
use vpx_sys::vp8e_enc_control_id::*;
use vpx_sys::vpx_codec_cx_pkt_kind::VPX_CODEC_CX_FRAME_PKT;
use vpx_sys::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum VideoCodecId {
    VP8,
    #[cfg(feature = "vp9")]
    VP9,
}

impl Default for VideoCodecId {
    #[cfg(not(feature = "vp9"))]
    fn default() -> VideoCodecId {
        VideoCodecId::VP8
    }

    #[cfg(feature = "vp9")]
    fn default() -> VideoCodecId {
        VideoCodecId::VP9
    }
}

pub struct Encoder {
    ctx: vpx_codec_ctx_t,
    pub width: usize,
    pub height: usize,
}

#[derive(Debug, Error)]
#[error("VPX encode error: {msg}")]
pub struct Error {
    msg: String,
    #[cfg(feature = "backtrace")]
    #[backtrace]
    backtrace: Backtrace,
}

impl From<String> for Error {
    fn from(msg: String) -> Self {
        Self {
            msg,
            #[cfg(feature = "backtrace")]
            backtrace: Backtrace::capture(),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

macro_rules! call_vpx {
    ($x:expr) => {{
        let result = unsafe { $x }; // original expression
        let result_int = unsafe { std::mem::transmute::<_, i32>(result) };
        // if result != VPX_CODEC_OK {
        if result_int != 0 {
            let code = unsafe { std::ffi::CStr::from_ptr(vpx_codec_err_to_string(result)) };
            let code = code.to_str().unwrap();

            return Err(Error::from(format!("VPX function call failed ({code}).")));
        }
        result
    }};
}

macro_rules! call_vpx_ptr {
    ($x:expr) => {{
        let result = unsafe { $x }; // original expression
        if result.is_null() {
            return Err(Error::from("Bad pointer.".to_string()));
        }
        result
    }};
}

unsafe impl Send for Encoder {}

impl Encoder {
    pub fn new(config: Config) -> Result<Self> {
        let i = match config.codec {
            VideoCodecId::VP8 => call_vpx_ptr!(vpx_codec_vp8_cx()),
            #[cfg(feature = "vp9")]
            VideoCodecId::VP9 => call_vpx_ptr!(vpx_codec_vp9_cx()),
        };

        if config.width % 2 != 0 {
            return Err(Error::from("Width must be divisible by 2".to_string()));
        }
        if config.height % 2 != 0 {
            return Err(Error::from("Height must be divisible by 2".to_string()));
        }

        let c = MaybeUninit::zeroed();
        let mut c = unsafe { c.assume_init() };
        call_vpx!(vpx_codec_enc_config_default(i, &mut c, 0));

        c.g_w = config.width;
        c.g_h = config.height;
        c.g_timebase.num = config.timebase[0];
        c.g_timebase.den = config.timebase[1];
        c.rc_target_bitrate = config.bitrate;

        c.g_threads = 8;
        c.g_error_resilient = VPX_ERROR_RESILIENT_DEFAULT;

        let ctx = MaybeUninit::zeroed();
        let mut ctx = unsafe { ctx.assume_init() };

        match config.codec {
            VideoCodecId::VP8 => {
                call_vpx!(vpx_codec_enc_init_ver(
                    &mut ctx,
                    i,
                    &c,
                    0,
                    vpx_sys::VPX_ENCODER_ABI_VERSION as i32
                ));
            }
            #[cfg(feature = "vp9")]
            VideoCodecId::VP9 => {
                call_vpx!(vpx_codec_enc_init_ver(
                    &mut ctx,
                    i,
                    &c,
                    0,
                    vpx_sys::VPX_ENCODER_ABI_VERSION as i32
                ));
                // set encoder internal speed settings
                call_vpx!(vpx_codec_control_(
                    &mut ctx,
                    VP8E_SET_CPUUSED as _,
                    6 as c_int
                ));
                // set row level multi-threading
                call_vpx!(vpx_codec_control_(
                    &mut ctx,
                    VP9E_SET_ROW_MT as _,
                    1 as c_int
                ));
            }
        };

        Ok(Self {
            ctx,
            width: config.width as usize,
            height: config.height as usize,
        })
    }

    pub fn encode(&mut self, pts: i64, data: &[u8], force_keyframe: bool) -> Result<Packets> {
        // assert!(2 * data.len() >= 3 * self.width * self.height);

        let image = MaybeUninit::zeroed();
        let mut image = unsafe { image.assume_init() };

        call_vpx_ptr!(vpx_img_wrap(
            &mut image,
            vpx_img_fmt::VPX_IMG_FMT_I420,
            // vpx_img_fmt::VPX_IMG_FMT_NV12,
            self.width as _,
            self.height as _,
            1,
            data.as_ptr() as _,
        ));

        let flags = if force_keyframe {
            VPX_EFLAG_FORCE_KF
        } else {
            0
        };

        // println!("vpx_image {:#?}", image);

        call_vpx!(vpx_codec_encode(
            &mut self.ctx,
            &image,
            pts,
            33, // Duration
            flags as i64,
            vpx_sys::VPX_DL_REALTIME as c_ulong,
        ));

        Ok(Packets {
            ctx: &mut self.ctx,
            iter: ptr::null(),
        })
    }

    pub fn finish(mut self) -> Result<Finish> {
        call_vpx!(vpx_codec_encode(
            &mut self.ctx,
            ptr::null(),
            -1, // PTS
            1,  // Duration
            0,  // Flags
            vpx_sys::VPX_DL_REALTIME as c_ulong,
        ));

        Ok(Finish {
            enc: self,
            iter: ptr::null(),
        })
    }

    pub fn wrap_image(&self, data: &[u8], width: u32, height: u32) -> Result<ImageWrap> {
        let image = MaybeUninit::zeroed();
        let mut image = unsafe { image.assume_init() };

        call_vpx_ptr!(vpx_img_wrap(
            &mut image,
            vpx_img_fmt::VPX_IMG_FMT_I420,
            width,
            height,
            1,
            data.as_ptr() as _,
        ));

        Ok(ImageWrap(image))
    }
}

pub struct ImageWrap(vpx_image);

impl ImageWrap {
    pub fn schmoo(&mut self) {
        println!("vpx_image planes {:?}", self.0.planes);
        println!("          stride {:?}", self.0.stride);
    }

    pub fn get_planes(&self) -> ([*mut u8; 4], [i32; 4], [u32; 4], [u32; 4]) {
        let vpx_image {
            planes,
            stride,
            w,
            h,
            ..
        } = self.0;
        let w2 = w / 2;
        let h2 = h / 2;
        (planes, stride, [w, w2, w2, w2], [h, h2, h2, h2])
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe {
            let result = vpx_codec_destroy(&mut self.ctx);
            if result != vpx_sys::VPX_CODEC_OK {
                eprintln!("failed to destroy vpx codec: {result:?}");
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Frame<'a> {
    /// Compressed data.
    pub data: &'a [u8],
    /// Whether the frame is a keyframe.
    pub key: bool,
    /// Presentation timestamp (in timebase units).
    pub pts: i64,
}

#[derive(Clone, Copy, Debug)]
pub struct Config {
    /// The width (in pixels).
    pub width: c_uint,
    /// The height (in pixels).
    pub height: c_uint,
    /// The timebase numerator and denominator (in seconds).
    pub timebase: [c_int; 2],
    /// The target bitrate (in kilobits per second).
    pub bitrate: c_uint,
    /// The codec
    pub codec: VideoCodecId,
}

pub struct Packets<'a> {
    ctx: &'a mut vpx_codec_ctx_t,
    iter: vpx_codec_iter_t,
}

impl<'a> Iterator for Packets<'a> {
    type Item = Frame<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            unsafe {
                // https://chromium.googlesource.com/webm/libvpx/+/mcw2/vpx/vpx_encoder.h
                // ```
                // * The data buffers returned from this function are only guaranteed to be
                // * valid until the application makes another call to any vpx_codec_* function.
                // ```
                let pkt = vpx_codec_get_cx_data(self.ctx, &mut self.iter);
                if pkt.is_null() {
                    return None;
                } else if (*pkt).kind == VPX_CODEC_CX_FRAME_PKT {
                    let f = &(*pkt).data.frame;
                    return Some(Frame {
                        data: slice::from_raw_parts(f.buf as _, f.sz as usize),
                        key: (f.flags & VPX_FRAME_IS_KEY) != 0,
                        pts: f.pts,
                    });
                } else {
                    // Ignore the packet.
                }
            }
        }
    }
}

pub struct Finish {
    enc: Encoder,
    iter: vpx_codec_iter_t,
}

impl Finish {
    pub fn next(&mut self) -> Result<Option<Frame>> {
        let mut tmp = Packets {
            ctx: &mut self.enc.ctx,
            iter: self.iter,
        };

        if let Some(packet) = tmp.next() {
            self.iter = tmp.iter;
            Ok(Some(packet))
        } else {
            call_vpx!(vpx_codec_encode(
                tmp.ctx,
                ptr::null(),
                -1, // PTS
                1,  // Duration
                0,  // Flags
                vpx_sys::VPX_DL_REALTIME as c_ulong,
            ));

            tmp.iter = ptr::null();
            if let Some(packet) = tmp.next() {
                self.iter = tmp.iter;
                Ok(Some(packet))
            } else {
                Ok(None)
            }
        }
    }
}
