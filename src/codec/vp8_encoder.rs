use std::mem::MaybeUninit;
use std::{ptr, slice};

use vpx_sys::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Vp8Config {
    pub width: u32,
    pub height: u32,
    pub timebase: [i32; 2],
    pub bitrate: u32,
}

impl Vp8Config {
    pub fn new(width: u32, height: u32, timebase: [i32; 2], bitrate: u32) -> Result<Self> {
        if width % 2 != 0 {
            return Err(Error::InvalidParam("width must be even"));
        };
        if height % 2 != 0 {
            return Err(Error::InvalidParam("height must be even"));
        };
        Ok(Self {
            width,
            height,
            timebase,
            bitrate,
        })
    }
}

pub struct Vp8Encoder {
    context: vpx_codec_ctx,
    config: Vp8Config,
}

unsafe impl Send for Vp8Encoder {}

impl Vp8Encoder {
    pub fn new(config: &Vp8Config) -> Result<Self> {
        let interface = vp8_interface()?;
        let mut vpx_config = default_encoder_config(interface)?;

        set_encoder_config(&mut vpx_config, config);
        vpx_config.g_threads = 8;
        vpx_config.g_error_resilient = VPX_ERROR_RESILIENT_DEFAULT;

        let context = create_vp8_context(interface, &vpx_config)?;

        Ok(Self {
            context,
            config: *config,
        })
    }

    /// Only YV12, I420 and NV12 images are supported.
    pub fn encode(
        &mut self,
        pts: i64,
        image: vpx_image,
        force_keyframe: bool,
    ) -> Result<Vp8EncoderData> {
        let flags = if force_keyframe {
            Vp8Flags::FORCE_KF
        } else {
            Vp8Flags::empty()
        };
        encode_image(&mut self.context, &image, pts, 33, flags)?;

        Ok(Vp8EncoderData::new(self))
    }

    pub fn config(&self) -> &Vp8Config {
        &self.config
    }

    pub fn wrap_image(&self, data: &[u8], format: ImageFormat) -> Result<vpx_image> {
        create_image_wrap(self.config.width, self.config.height, data, format)
    }
}

pub struct Vp8EncoderData<'enc> {
    encoder: &'enc mut Vp8Encoder,
    iterator: vpx_codec_iter_t,
}

unsafe impl<'enc> Send for Vp8EncoderData<'enc> {}

impl<'enc> Vp8EncoderData<'enc> {
    fn new(encoder: &'enc mut Vp8Encoder) -> Self {
        Self {
            encoder,
            iterator: ptr::null(),
        }
    }

    pub fn frames(&mut self) -> impl Iterator<Item = Vp8Frame> {
        std::iter::from_fn(|| loop {
            let Some(packet) = next_packet(&mut self.encoder.context, &mut self.iterator) else {
                return None
            };

            match packet.kind {
                vpx_codec_cx_pkt_kind::VPX_CODEC_CX_FRAME_PKT => {
                    return Some(unsafe { Vp8Frame::new(packet) })
                }
                vpx_codec_cx_pkt_kind::VPX_CODEC_STATS_PKT => todo!(),
                vpx_codec_cx_pkt_kind::VPX_CODEC_FPMB_STATS_PKT => todo!(),
                vpx_codec_cx_pkt_kind::VPX_CODEC_PSNR_PKT => todo!(),
                vpx_codec_cx_pkt_kind::VPX_CODEC_CUSTOM_PKT => todo!(),
            }
        })
    }
}

pub struct Vp8Frame<'data> {
    pub data: &'data [u8],
    pub pts: i64,
    pub duration: u64,
    pub width: u32,
    pub height: u32,

    flags: InternalFrameFlags,
}

impl Vp8Frame<'_> {
    unsafe fn new(packet: &vpx_codec_cx_pkt) -> Self {
        let frame = unsafe { &packet.data.frame };
        let data = unsafe { slice::from_raw_parts(frame.buf as _, frame.sz as usize) };
        let pts = frame.pts;
        let duration = frame.duration;
        let flags = InternalFrameFlags::from_bits_truncate(frame.flags);
        let width = frame.width[0];
        let height = frame.height[0];
        // * .partition_id not supported since partitioned frames are not supported for now
        // * only consider layer 0 because VP8 only uses this one, ignore .spatial_layer_encoded completely
        Self {
            data,
            pts,
            duration,
            flags,
            width,
            height,
        }
    }

    pub fn keyframe(&self) -> bool {
        self.flags.contains(InternalFrameFlags::IS_KEY)
    }
}

impl Drop for Vp8Encoder {
    fn drop(&mut self) {
        let result = unsafe { vpx_codec_destroy(&mut self.context) };
        if result != vpx_sys::VPX_CODEC_OK {
            eprintln!("failed to destroy vpx codec: {result:?}");
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("")]
    VP8Unsupported,
    #[error("")]
    InvalidParam(&'static str),
    #[error("")]
    ImageWrapNotCreated,

    // TODO we can be more specific than this
    #[error("")]
    Bad,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(unused)]
pub enum ImageFormat {
    /// Same as f420 on Mac
    I420,
    YV12,
    /// Same as 420v on Mac
    NV12,
}

impl ImageFormat {
    fn vpx_img_fmt(&self) -> vpx_img_fmt {
        match self {
            ImageFormat::I420 => vpx_img_fmt::VPX_IMG_FMT_I420,
            ImageFormat::YV12 => vpx_img_fmt::VPX_IMG_FMT_YV12,
            ImageFormat::NV12 => vpx_img_fmt::VPX_IMG_FMT_NV12,
        }
    }
}

// TODO test and consider using libvpx error strings
//
// fn vpx_error_to_string(error: vpx_codec_err_t) -> Cow<'static, str> {
//     let string = unsafe { std::ffi::CStr::from_ptr(vpx_codec_err_to_string(error)) };
//     string.to_string_lossy()
// }

fn vp8_interface() -> Result<&'static mut vpx_codec_iface> {
    unsafe { vpx_codec_vp8_cx().as_mut() }.ok_or(Error::VP8Unsupported)
}

fn default_encoder_config(interface: &mut vpx_codec_iface) -> Result<vpx_codec_enc_cfg> {
    let mut vpx_config = MaybeUninit::zeroed();
    let result = unsafe {
        vpx_codec_enc_config_default(interface as *mut _, vpx_config.assume_init_mut(), 0)
    };
    if result != VPX_CODEC_OK {
        Err(Error::Bad)
    } else {
        Ok(unsafe { vpx_config.assume_init() })
    }
}

fn set_encoder_config(vpx_config: &mut vpx_codec_enc_cfg, config: &Vp8Config) {
    vpx_config.g_w = config.width;
    vpx_config.g_h = config.height;
    vpx_config.g_timebase.num = config.timebase[0];
    vpx_config.g_timebase.den = config.timebase[1];
    vpx_config.rc_target_bitrate = config.bitrate;
}

fn create_vp8_context(
    interface: &mut vpx_codec_iface,
    vpx_config: &vpx_codec_enc_cfg,
) -> Result<vpx_codec_ctx> {
    let mut context = MaybeUninit::zeroed();
    let result = unsafe {
        vpx_codec_enc_init_ver(
            context.assume_init_mut(),
            interface,
            vpx_config,
            0,
            vpx_sys::VPX_ENCODER_ABI_VERSION as _,
        )
    };
    if result != VPX_CODEC_OK {
        Err(Error::Bad)
    } else {
        Ok(unsafe { context.assume_init() })
    }
}

fn create_image_wrap(
    width: u32,
    height: u32,
    data: &[u8],
    format: ImageFormat,
) -> Result<vpx_image> {
    let mut image = MaybeUninit::zeroed();
    let stride_align = 1;
    let result = unsafe {
        vpx_img_wrap(
            image.assume_init_mut(),
            format.vpx_img_fmt(),
            width,
            height,
            stride_align,
            data.as_ptr() as _,
        )
    };
    if result.is_null() {
        Err(Error::ImageWrapNotCreated)
    } else {
        Ok(unsafe { image.assume_init() })
    }
}

fn encode_image(
    context: &mut vpx_codec_ctx,
    image: &vpx_image,
    pts: vpx_codec_pts_t,
    duration: u64,
    flags: Vp8Flags,
) -> Result<()> {
    let result = unsafe {
        vpx_codec_encode(
            context,
            image,
            pts,
            duration,
            flags.bits as i64,
            vpx_sys::VPX_DL_REALTIME as u64,
        )
    };
    if result != VPX_CODEC_OK {
        Err(Error::Bad)
    } else {
        Ok(())
    }
}

fn next_packet<'iter>(
    context: &mut vpx_codec_ctx,
    iter: &'iter mut vpx_codec_iter_t,
) -> Option<&'iter vpx_codec_cx_pkt> {
    unsafe { vpx_codec_get_cx_data(context, iter).as_ref() }
}

bitflags::bitflags! {
    struct InternalFrameFlags: u32 {
        const IS_KEY = VPX_FRAME_IS_KEY;
        const IS_DROPPABLE = VPX_FRAME_IS_DROPPABLE;
        const IS_VISIBLE = VPX_FRAME_IS_INVISIBLE;
        const IS_FRAGMENT = VPX_FRAME_IS_FRAGMENT;
    }
}

bitflags::bitflags! {
    struct Vp8Flags: u32 {
        const FORCE_KF = VPX_EFLAG_FORCE_KF;

        const NO_REF_LAST = VP8_EFLAG_NO_REF_LAST;
        const NO_REF_GF = VP8_EFLAG_NO_REF_GF;
        // TODO see vp8cx.h to add more
    }
}
