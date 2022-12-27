use std::os::raw::c_char;

pub use std::ffi::c_void;
pub use std::ptr::null;

#[link(name = "AVFoundation", kind = "framework")]
extern "C" {
    pub static AVVideoExpectedSourceFrameRateKey: Id<NSString, Shared>;
    pub static AVVideoMaxKeyFrameIntervalDurationKey: Id<NSString, Shared>;
}

// libdispatch is loaded differently on MacOS and iOS. Have a look in https://docs.rs/dispatch
// We don't care about the exact types.
#[link(name = "System", kind = "dylib")]
extern "C" {
    pub fn dispatch_queue_create(name: *const c_char, attr: *const c_void) -> DispatchQueueT;
    pub fn dispatch_release(queue: DispatchQueueT);
}
type DispatchQueueT = *mut NSObject;

#[link(name = "CoreVideo", kind = "framework")]
extern "C" {
    pub fn CVPixelBufferLockBaseAddress(buf: CVBufferRef, option: u64) -> i32;
    pub fn CVPixelBufferUnlockBaseAddress(buf: CVBufferRef, option: u64) -> i32;
    pub fn CVPixelBufferGetBaseAddress(buf: CVBufferRef) -> *const u8;
    pub fn CVPixelBufferGetBytesPerRow(buf: CVBufferRef) -> usize;
    pub fn CVPixelBufferGetWidth(buf: CVBufferRef) -> usize;
    pub fn CVPixelBufferGetHeight(buf: CVBufferRef) -> usize;
    pub fn CVPixelBufferIsPlanar(buf: CVBufferRef) -> bool;
    pub fn CVPixelBufferGetPlaneCount(buf: CVBufferRef) -> usize;
    pub fn CVPixelBufferGetHeightOfPlane(buf: CVBufferRef, index: usize) -> usize;
    pub fn CVPixelBufferGetBytesPerRowOfPlane(buf: CVBufferRef, index: usize) -> usize;
    pub fn CVPixelBufferGetDataSize(buf: CVBufferRef) -> usize;
    pub fn CVPixelBufferGetPixelFormatType(buf: CVBufferRef) -> u32;
    pub fn CVPixelBufferGetBaseAddressOfPlane(buf: CVBufferRef, index: usize) -> *const u8;
}

#[link(name = "CoreMedia", kind = "framework")]
extern "C" {
    pub fn CMSampleBufferGetFormatDescription(
        sbuf: *const CMSampleBuffer,
    ) -> *const CMFormatDescription;
    pub fn CMSampleBufferGetImageBuffer(sbuf: *const CMSampleBuffer) -> CVImageBufferRef;
    pub fn CMFormatDescriptionGetMediaSubType(desc: *const CMFormatDescription) -> u32;
    pub fn CMVideoFormatDescriptionGetDimensions(
        desc: *const CMFormatDescription,
    ) -> CMVideoDimensions;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    pub fn CFRetain(cf: *const c_void) -> *const c_void;
    pub fn CFRelease(cf: *const c_void);
}

/// Use when you need to translate typedefs like this:
/// typedef struct opaqueCMSampleBuffer CMSampleBufferRef;
macro_rules! opaque_struct {
    ($opaque_name:ident, $ref_name:ident) => {
        #[repr(C)]
        pub struct $ref_name {
            _priv: [u8; 0],
        }
        unsafe impl Encode for $ref_name {
            const ENCODING: Encoding = Encoding::Struct(stringify!($opaque_name), &[]);
        }
        unsafe impl RefEncode for $ref_name {
            const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
        }
    };
}

opaque_struct!(opaqueCMSampleBuffer, CMSampleBuffer);
opaque_struct!(opaqueCMFormatDescription, CMFormatDescription);

#[repr(C)]
pub struct CVBuffer {
    _priv: [u8; 0],
}
pub type CVBufferRef = *const CVBuffer;
pub type CVImageBufferRef = CVBufferRef;

#[repr(C)]
#[derive(Debug)]
pub struct CMVideoDimensions {
    pub width: i32,
    pub height: i32,
}

/*  */

use icrate::{
    ns_string,
    objc2::{declare_class, extern_class, rc::*, runtime::*, *},
    Foundation::*,
};

fn foo() {
    let x: NSString;
}

extern_class! {
    pub struct AVFrameRateRange;

    unsafe impl ClassType for AVFrameRateRange {
        type Super = NSObject;
    }
}

extern_class! {
    pub struct AVCaptureDeviceFormat;

    unsafe impl ClassType for AVCaptureDeviceFormat {
        type Super = NSObject;
    }
}

extern_class! {
    #[derive(PartialEq, Eq, Hash, Debug)]
    pub struct AVCaptureDevice;

    unsafe impl ClassType for AVCaptureDevice {
        type Super = NSObject;
    }
}

impl AVCaptureDevice {
    pub fn default_video() -> Option<Id<Self, Shared>> {
        let video = ns_string!("vide");
        unsafe { msg_send_id![Self::class(), defaultDeviceWithMediaType: video] }
    }

    pub fn localized_name(&self) -> Id<NSString, Shared> {
        unsafe { msg_send_id![self, localizedName] }
    }

    pub fn formats(&self) -> Id<NSArray<AVCaptureDeviceFormat>, Shared> {
        unsafe { msg_send_id![self, formats] }
    }
}

impl AVCaptureDeviceFormat {
    pub fn format_description(&self) -> *const CMFormatDescription {
        unsafe { msg_send![self, formatDescription] }
    }

    pub fn video_supported_framerate_ranges(&self) -> Id<NSArray<AVFrameRateRange>, Shared> {
        unsafe { msg_send_id![self, videoSupportedFrameRateRanges] }
    }
}

/// var maxFrameDuration: CMTime
/// var maxFrameRate: Float64
/// var minFrameDuration: CMTime
/// var minFrameRate: Float64
impl AVFrameRateRange {
    pub fn max_framerate(&self) -> f64 {
        unsafe { msg_send![self, maxFrameRate] }
    }

    pub fn min_framerate(&self) -> f64 {
        unsafe { msg_send![self, minFrameRate] }
    }
}

extern_class! {
    pub struct AVCaptureDeviceInput;

    unsafe impl ClassType for AVCaptureDeviceInput {
        type Super = NSObject; // it is really AVCaptureInput
    }
}

impl AVCaptureDeviceInput {
    pub fn from_device(
        device: &AVCaptureDevice,
    ) -> std::result::Result<Id<Self, Shared>, Id<NSError, Shared>> {
        unsafe { msg_send_id![Self::class(), deviceInputWithDevice: device, error: _] }
    }
}

objc2::extern_class! {
    pub struct AVCaptureSession;

    unsafe impl ClassType for AVCaptureSession {
        type Super = NSObject;
    }
}

impl AVCaptureSession {
    pub fn new() -> Id<Self, Shared> {
        unsafe { msg_send_id![Self::class(), new] }
    }

    pub fn can_add_input(&self, input: &AVCaptureDeviceInput) -> bool {
        unsafe { msg_send![self, canAddInput: input] }
    }

    pub fn add_input(&self, input: &AVCaptureDeviceInput) {
        unsafe { msg_send![self, addInput: input] }
    }

    pub fn can_add_output(&self, output: &AVCaptureVideoDataOutput) -> bool {
        unsafe { msg_send![self, canAddOutput: output] }
    }

    pub fn add_output(&self, output: &AVCaptureVideoDataOutput) {
        unsafe { msg_send![self, addOutput: output] }
    }

    pub fn start_running(&self) {
        unsafe { msg_send![self, startRunning] }
    }

    pub fn stop_running(&self) {
        unsafe { msg_send![self, stopRunning] }
    }
}

objc2::extern_class! {
    pub struct AVCaptureVideoDataOutput;

    unsafe impl ClassType for AVCaptureVideoDataOutput {
        type Super = NSObject;
    }
}

impl AVCaptureVideoDataOutput {
    pub fn new() -> Id<Self, Owned> {
        unsafe { msg_send_id![Self::class(), new] }
    }

    /// Returns vector of fourcc u32
    ///
    /// Corresponds to availableVideoCVPixelFormatTypes
    #[allow(dead_code)] // TODO expose this info
    pub fn available_video_pixel_format_types(&self) -> Vec<u32> {
        let px_formats: Id<NSArray<NSNumber>, Shared> =
            unsafe { msg_send_id![self, availableVideoCVPixelFormatTypes] };
        px_formats.iter().map(|num| num.as_u32()).collect()
    }
}

extern_methods! {
    unsafe impl AVCaptureVideoDataOutput {
        #[method(setVideoSettings:)]
        fn set_video_settings(&mut self, settings: &NSDictionary<NSString, NSNumber>);

        #[method(setSampleBufferDelegate:queue:)]
        fn set_sample_buffer_delegate(&mut self, delegate: &NSObject, queue: DispatchQueueT);
    }
}

type CallbackPtr = *const c_void;

declare_class!(
    pub struct MyVideoDataOutputDelegate {
        pub callback: CallbackPtr,
    }

    unsafe impl ClassType for MyVideoDataOutputDelegate {
        type Super = NSObject;
    }

    unsafe impl MyVideoDataOutputDelegate {
        #[method(initWithCallback:)]
        fn init_with(&mut self, callback: CallbackPtr) -> Option<&mut Self> {
            let this: Option<&mut Self> = unsafe { msg_send![super(self), init] };
            this.map(|this| {
                *this.callback = callback;
                this
            })
        }

        #[method(callback)]
        fn __callback(&self) -> CallbackPtr {
            *self.callback
        }

        #[method(captureOutput:didOutputSampleBuffer:fromConnection:)]
        fn __capture(
            &self,
            _output: *const c_void,
            sample: *const c_void,
            _connection: *const c_void,
        ) {
            let callback: *const c_void = *self.callback;
            let delegate = unsafe { &*callback.cast::<Box<dyn VideoDataOutputDelegate>>() };
            delegate.frame(sample as *const CMSampleBuffer);
        }
    }
);

impl MyVideoDataOutputDelegate {
    #[allow(clippy::borrowed_box)]
    pub fn new(callback: &Box<dyn VideoDataOutputDelegate>) -> Id<Self, Owned> {
        let ptr = callback as *const _ as *const c_void;
        let cls = Self::class();
        unsafe { msg_send_id![msg_send_id![cls, alloc], initWithCallback: ptr] }
    }
}

pub trait VideoDataOutputDelegate {
    fn frame(&self, sbuf: *const CMSampleBuffer);
}
/* */

use std::{io::Result, sync::mpsc};

/// A camera device. Use it to get and find out about a device and capture [frames](Frame).
pub struct Camera {
    name: String,
    device: Id<AVCaptureDevice, Shared>,
    capture: Id<AVCaptureSession, Shared>,
    frame_sender: Box<dyn VideoDataOutputDelegate>,
    frame_receiver: mpsc::Receiver<Frame>,
    delegate: Option<Id<MyVideoDataOutputDelegate, Owned>>,
    prefererred_format: Option<DeviceFormat>,
}

impl Camera {
    pub fn default() -> Result<Self> {
        let device = AVCaptureDevice::default_video().unwrap();
        let name = device.localized_name().to_string();
        let (frame_sender, frame_receiver) = mpsc::sync_channel(1);

        Ok(Self {
            name,
            device,
            capture: AVCaptureSession::new(),
            frame_sender: Box::new(FrameSender {
                sender: frame_sender,
            }),
            frame_receiver,
            delegate: None,
            prefererred_format: None,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn formats(&self) -> Vec<DeviceFormat> {
        self.device
            .formats()
            .iter()
            .map(DeviceFormat::new)
            .collect()
    }

    pub fn set_preferred_format(&mut self, format: Option<DeviceFormat>) {
        self.prefererred_format = format;
    }

    pub fn start(&mut self) -> Result<()> {
        let input = AVCaptureDeviceInput::from_device(&self.device).unwrap();
        let mut output = AVCaptureVideoDataOutput::new();

        let video_settings = self.video_settings(&Config {});
        output.set_video_settings(&video_settings);

        self.delegate = Some(MyVideoDataOutputDelegate::new(&self.frame_sender));
        let delegate = self.delegate.as_ref().unwrap();

        let name = std::ffi::CString::new("video input").unwrap();
        // Calling create, setSampleBufferDelegate and release like I saw in ffmpeg
        // https://github.com/FFmpeg/FFmpeg/blob/master/libavdevice/avfoundation.m
        let queue = unsafe { dispatch_queue_create(name.as_ptr(), null()) };

        output.set_sample_buffer_delegate(delegate, queue);

        unsafe { dispatch_release(queue) };

        assert!(self.capture.can_add_input(&input));
        self.capture.add_input(&input);

        assert!(self.capture.can_add_output(&output));
        self.capture.add_output(&output);

        self.capture.start_running();
        Ok(())
    }

    pub fn stop(&mut self) {
        self.capture.stop_running();
    }

    pub fn frames(&self) -> std::sync::mpsc::Iter<Frame> {
        self.frame_receiver.iter()
    }

    fn video_settings(
        &self,
        _config: &Config,
    ) -> Id<NSMutableDictionary<NSString, NSNumber>, Owned> {
        return if let Some(format) = &self.prefererred_format {
            video_settings_with_pixel_format(str_to_u32(&format.pixel_format))
        } else {
            let rgba = 0x20;
            video_settings_with_pixel_format(rgba)
        };

        fn str_to_u32(string: &str) -> u32 {
            assert_eq!(4, string.len());
            let bytes = string.as_bytes();
            let a = bytes[0];
            let b = bytes[1];
            let c = bytes[2];
            let d = bytes[3];
            unsafe { std::mem::transmute::<[u8; 4], u32>([a, b, c, d]) }.to_be()
        }

        fn video_settings_with_pixel_format(
            pixel_format: u32,
        ) -> Id<NSMutableDictionary<NSString, NSNumber>, Owned> {
            let mut settings = NSMutableDictionary::<NSString, NSNumber>::new();
            let px_number = NSNumber::new_u32(pixel_format);
            let px_format_type = NSString::from_str("PixelFormatType"); // kCVPixelBufferPixelFormatTypeKey
            unsafe { settings.insert(px_format_type, Id::from_shared(px_number)) };
            settings
        }
    }
}

/// Not implemented. 🤷
/// Configure a [camera](Camera) device to capture specific frame sizes, frame rates and different pixel formats.
pub struct Config {
    // pub interval: (u32, u32),
    // pub resolution: (u32, u32),
    // pub format: &'a [u8],
}

struct FrameSender {
    sender: mpsc::SyncSender<Frame>,
}

impl VideoDataOutputDelegate for FrameSender {
    fn frame(&self, sbuf: *const CMSampleBuffer) {
        let _ = self.sender.try_send(Frame::new(sbuf));
    }
}

/// Holds the frame data without copying it and releases it upon drop.
/// You can find out about the [format](Frame::format) and get a locked reference to the [pixel data](Frame::pixels).
pub struct Frame {
    sbuf: &'static CMSampleBuffer,
}

impl std::fmt::Debug for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Frame").finish()
    }
}

impl Frame {
    fn new(ptr: *const CMSampleBuffer) -> Self {
        Self {
            sbuf: Self::retain(ptr),
        }
    }

    fn retain(ptr: *const CMSampleBuffer) -> &'static CMSampleBuffer {
        let ptr = unsafe { CFRetain(ptr as *const _) as *const CMSampleBuffer };
        unsafe { ptr.as_ref().unwrap() }
    }

    pub fn format(&self) -> SampleFormat {
        SampleFormat::new(self)
    }

    pub fn pixels(&self) -> Pixels {
        Pixels::new(self)
    }

    pub fn raw_sample_buffer(&self) -> *const CMSampleBuffer {
        self.sbuf
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        let ptr = self.raw_sample_buffer() as *const _;
        unsafe { CFRelease(ptr) };
    }
}

/// Holds the locked pixel data of a frame and unlocks upon drop.
pub struct Pixels<'a> {
    pub ibuf: CVImageBufferRef,
    pub data: &'a [u8],
}

impl<'a> Pixels<'a> {
    fn new(frame: &'a Frame) -> Self {
        let ibuf = unsafe { CMSampleBufferGetImageBuffer(frame.sbuf) };
        debug_assert!(0 == unsafe { CVPixelBufferLockBaseAddress(ibuf, 1) });
        let _address = unsafe { CVPixelBufferGetBaseAddress(ibuf) };
        let stride = unsafe { CVPixelBufferGetBytesPerRow(ibuf) };
        let _width = unsafe { CVPixelBufferGetWidth(ibuf) };
        let height = unsafe { CVPixelBufferGetHeight(ibuf) };
        let is_planar = unsafe { CVPixelBufferIsPlanar(ibuf) };
        let plane_count = unsafe { CVPixelBufferGetPlaneCount(ibuf) };
        let _data_size = unsafe { CVPixelBufferGetDataSize(ibuf) };
        let _fourcc = unsafe { CVPixelBufferGetPixelFormatType(ibuf) };
        let plane_address = unsafe { CVPixelBufferGetBaseAddressOfPlane(ibuf, 0) };
        let mut plane_sizes = 0;

        // println!("pixels {:?}", (address, stride, width, height, is_planar, plane_count, data_size, fourcc_to_string(fourcc)));
        if is_planar {
            for index in 0..plane_count {
                let _plane_address = unsafe { CVPixelBufferGetBaseAddressOfPlane(ibuf, index) };
                let plane_stride = unsafe { CVPixelBufferGetBytesPerRowOfPlane(ibuf, index) };
                let plane_height = unsafe { CVPixelBufferGetHeightOfPlane(ibuf, index) };
                // println!("        {:?}", (plane_address, plane_stride, plane_height));
                plane_sizes += plane_stride * plane_height;
            }
        } else {
            plane_sizes += stride * height;
        }

        let data = unsafe { std::slice::from_raw_parts(plane_address, plane_sizes) };
        Self { ibuf, data }
    }
}

impl Drop for Pixels<'_> {
    fn drop(&mut self) {
        debug_assert!(0 == unsafe { CVPixelBufferUnlockBaseAddress(self.ibuf, 1) });
    }
}

/// A specific resolution, framerate and pixel format supported by a [camera](Camera) device.
#[derive(Debug, Clone)]
pub struct DeviceFormat {
    pub width: i32,
    pub height: i32,
    pub max_framerate: f64,
    pub pixel_format: String,
}

impl DeviceFormat {
    pub fn new(format: &AVCaptureDeviceFormat) -> Self {
        let format_desc = format.format_description();
        let dim = unsafe { CMVideoFormatDescriptionGetDimensions(format_desc) };
        let fourcc = unsafe { CMFormatDescriptionGetMediaSubType(format_desc) };
        let max_framerate = format
            .video_supported_framerate_ranges()
            .iter()
            .map(|range| range.max_framerate())
            .max_by(f64::total_cmp)
            .unwrap_or(0.0);

        Self {
            width: dim.width,
            height: dim.height,
            max_framerate,
            pixel_format: fourcc_to_string(fourcc),
        }
    }
}

impl std::fmt::Display for DeviceFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            width,
            height,
            max_framerate,
            pixel_format,
        } = self;
        f.write_fmt(format_args!(
            "{width}x{height} @ {max_framerate:.2} {pixel_format}"
        ))
    }
}

/// The size and pixel format of a [frame](Frame).
#[derive(Debug)]
pub struct SampleFormat {
    pub width: i32,
    pub height: i32,
    pub pixel_format: String,
}

impl SampleFormat {
    pub fn new(frame: &Frame) -> Self {
        let format = unsafe { CMSampleBufferGetFormatDescription(frame.sbuf) };
        let dim = unsafe { CMVideoFormatDescriptionGetDimensions(format) };
        let fourcc = unsafe { CMFormatDescriptionGetMediaSubType(format) };
        let pixel_format = fourcc_to_string(fourcc);
        Self {
            width: dim.width,
            height: dim.height,
            pixel_format,
        }
    }
}

impl std::fmt::Display for SampleFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{}x{} {}",
            self.width, self.height, self.pixel_format
        ))
    }
}

/// FOURCC is a little crazy. Look at some references to interpret this obfuscation.
/// Look also into Chromium. There you can see that NV12 is a preferred format, 420v on Mac.
///
/// <https://chromium.googlesource.com/libyuv/libyuv/+/HEAD/docs/formats.md>
/// <https://softron.zendesk.com/hc/en-us/articles/207695697-List-of-FourCC-codes-for-video-codecs>
/// <http://abcavi.kibi.ru/fourcc.php>
pub fn fourcc_to_string(px_format_u32: u32) -> String {
    let bytes = px_format_u32.to_be_bytes();
    if bytes[0] == 0 {
        match px_format_u32 {
            32 => "ARGB",
            24 => "RGB ",
            _ => return format!("0x{px_format_u32:08X}"),
        }
        .into()
    } else {
        String::from_utf8_lossy(&bytes).to_string()
    }
}
