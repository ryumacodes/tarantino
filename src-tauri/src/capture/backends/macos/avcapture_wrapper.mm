// AVFoundation camera capture wrapper for Tarantino webcam recording
// Mirrors the ScreenCaptureKit wrapper pattern: ObjC callback → C callback → Rust

#import <AVFoundation/AVFoundation.h>
#import <CoreMedia/CoreMedia.h>
#import <CoreVideo/CoreVideo.h>
#import <Foundation/Foundation.h>
#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>

// Matches the Rust FFI struct layout
typedef struct {
    const uint8_t* data;
    size_t data_len;
    uint32_t width;
    uint32_t height;
    const char* pixel_format;
    uint64_t timestamp_us;
    uint32_t stride;
} AVCFrameData;

typedef struct {
    void* rust_context;
    void (*frame_callback)(void* context, AVCFrameData frame);
} AVCCallbackContext;

// ---- Preview window ----

@interface AVCPreviewWindow : NSWindow
@end

@implementation AVCPreviewWindow
- (BOOL)canBecomeKeyWindow { return NO; }
- (BOOL)canBecomeMainWindow { return NO; }
@end

// ---- Camera capture session ----

@interface AVCCameraSession : NSObject <AVCaptureVideoDataOutputSampleBufferDelegate>
@property (nonatomic, strong) AVCaptureSession* session;
@property (nonatomic, strong) AVCaptureDeviceInput* input;
@property (nonatomic, strong) AVCaptureVideoDataOutput* output;
@property (nonatomic, assign) AVCCallbackContext* callbackContext;
@property (nonatomic, assign) BOOL isCapturing;
@property (nonatomic, assign) BOOL isRecording; // true when encoding frames, false for preview-only
@property (nonatomic, assign) uint64_t frameCount;
@property (nonatomic, strong) AVCPreviewWindow* previewWindow;
@property (nonatomic, strong) AVCaptureVideoPreviewLayer* previewLayer;
@end

@implementation AVCCameraSession

- (instancetype)init {
    self = [super init];
    if (self) {
        _isCapturing = NO;
        _isRecording = NO;
        _frameCount = 0;
    }
    return self;
}

- (BOOL)startWithDeviceId:(NSString*)deviceId fps:(int)fps {
    // Find camera device
    AVCaptureDevice* device = nil;
    if (deviceId && deviceId.length > 0) {
        device = [AVCaptureDevice deviceWithUniqueID:deviceId];
    }
    if (!device) {
        device = [AVCaptureDevice defaultDeviceWithMediaType:AVMediaTypeVideo];
    }
    if (!device) {
        NSLog(@"[AVC] No camera device found");
        return NO;
    }
    NSLog(@"[AVC] Using camera: %@", device.localizedName);

    // Create capture session
    self.session = [[AVCaptureSession alloc] init];
    self.session.sessionPreset = AVCaptureSessionPreset1280x720;

    // Add input
    NSError* error = nil;
    self.input = [AVCaptureDeviceInput deviceInputWithDevice:device error:&error];
    if (error || !self.input) {
        NSLog(@"[AVC] Failed to create camera input: %@", error);
        return NO;
    }
    if ([self.session canAddInput:self.input]) {
        [self.session addInput:self.input];
    } else {
        NSLog(@"[AVC] Cannot add camera input to session");
        return NO;
    }

    // Configure camera frame rate
    [device lockForConfiguration:&error];
    if (!error) {
        device.activeVideoMinFrameDuration = CMTimeMake(1, fps);
        device.activeVideoMaxFrameDuration = CMTimeMake(1, fps);
        [device unlockForConfiguration];
    }

    // Add video data output (for recording frames to Rust)
    self.output = [[AVCaptureVideoDataOutput alloc] init];
    self.output.videoSettings = @{
        (NSString*)kCVPixelBufferPixelFormatTypeKey: @(kCVPixelFormatType_32BGRA)
    };
    self.output.alwaysDiscardsLateVideoFrames = YES;

    dispatch_queue_t queue = dispatch_queue_create("com.tarantino.webcam", DISPATCH_QUEUE_SERIAL);
    [self.output setSampleBufferDelegate:self queue:queue];

    if ([self.session canAddOutput:self.output]) {
        [self.session addOutput:self.output];
    } else {
        NSLog(@"[AVC] Cannot add video output to session");
        return NO;
    }

    // Start capture session
    [self.session startRunning];
    self.isCapturing = YES;
    NSLog(@"[AVC] Camera capture started");
    return YES;
}

- (void)showPreview {
    dispatch_async(dispatch_get_main_queue(), ^{
        if (self.previewWindow) return; // Already showing

        CGFloat size = 180.0;
        // Position in bottom-right corner of main screen
        NSScreen* screen = [NSScreen mainScreen];
        CGFloat x = NSMaxX(screen.visibleFrame) - size - 20;
        CGFloat y = NSMinY(screen.visibleFrame) + 20;
        NSRect frame = NSMakeRect(x, y, size, size);

        self.previewWindow = [[AVCPreviewWindow alloc]
            initWithContentRect:frame
                      styleMask:NSWindowStyleMaskBorderless
                        backing:NSBackingStoreBuffered
                          defer:NO];
        self.previewWindow.level = NSFloatingWindowLevel;
        self.previewWindow.backgroundColor = [NSColor clearColor];
        self.previewWindow.opaque = NO;
        self.previewWindow.hasShadow = YES;
        self.previewWindow.movableByWindowBackground = YES;
        self.previewWindow.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces;

        // Create content view with circular mask
        NSView* contentView = [[NSView alloc] initWithFrame:NSMakeRect(0, 0, size, size)];
        contentView.wantsLayer = YES;
        contentView.layer.cornerRadius = size / 2.0;
        contentView.layer.masksToBounds = YES;
        contentView.layer.borderColor = [[NSColor colorWithWhite:1.0 alpha:0.15] CGColor];
        contentView.layer.borderWidth = 2.0;

        // Add camera preview layer
        self.previewLayer = [AVCaptureVideoPreviewLayer layerWithSession:self.session];
        self.previewLayer.videoGravity = AVLayerVideoGravityResizeAspectFill;
        self.previewLayer.frame = contentView.bounds;
        // Mirror the preview (selfie-style)
        if (self.previewLayer.connection.isVideoMirroringSupported) {
            self.previewLayer.connection.automaticallyAdjustsVideoMirroring = NO;
            self.previewLayer.connection.videoMirrored = YES;
        }
        [contentView.layer addSublayer:self.previewLayer];

        self.previewWindow.contentView = contentView;
        [self.previewWindow orderFrontRegardless];

        NSLog(@"[AVC] Preview window shown");
    });
}

- (void)hidePreview {
    dispatch_async(dispatch_get_main_queue(), ^{
        if (self.previewWindow) {
            [self.previewWindow orderOut:nil];
            self.previewLayer = nil;
            self.previewWindow = nil;
            NSLog(@"[AVC] Preview window hidden");
        }
    });
}

- (void)startRecordingWithCallback:(AVCCallbackContext*)ctx {
    self.callbackContext = ctx;
    self.isRecording = YES;
    self.frameCount = 0;
    NSLog(@"[AVC] Frame recording started");
}

- (void)stopRecording {
    self.isRecording = NO;
    NSLog(@"[AVC] Frame recording stopped after %llu frames", self.frameCount);
}

- (void)stop {
    if (self.isCapturing) {
        [self hidePreview];
        [self.session stopRunning];
        self.isCapturing = NO;
        self.isRecording = NO;
        NSLog(@"[AVC] Camera capture stopped after %llu frames", self.frameCount);
    }
}

// AVCaptureVideoDataOutputSampleBufferDelegate
- (void)captureOutput:(AVCaptureOutput*)output
  didOutputSampleBuffer:(CMSampleBufferRef)sampleBuffer
         fromConnection:(AVCaptureConnection*)connection {

    // Only send frames to Rust when recording (not during preview-only)
    if (!self.isRecording || !self.callbackContext || !self.callbackContext->frame_callback) return;

    CVImageBufferRef imageBuffer = CMSampleBufferGetImageBuffer(sampleBuffer);
    if (!imageBuffer) return;

    CVPixelBufferLockBaseAddress(imageBuffer, kCVPixelBufferLock_ReadOnly);

    size_t width = CVPixelBufferGetWidth(imageBuffer);
    size_t height = CVPixelBufferGetHeight(imageBuffer);
    size_t stride = CVPixelBufferGetBytesPerRow(imageBuffer);
    void* baseAddress = CVPixelBufferGetBaseAddress(imageBuffer);
    size_t dataLen = stride * height;

    CMTime pts = CMSampleBufferGetPresentationTimeStamp(sampleBuffer);
    uint64_t timestamp_us = (uint64_t)((double)pts.value / (double)pts.timescale * 1000000.0);

    // Copy pixel data — buffer only valid during callback
    uint8_t* dataCopy = (uint8_t*)malloc(dataLen);
    if (dataCopy) {
        memcpy(dataCopy, baseAddress, dataLen);

        AVCFrameData frame = {
            .data = dataCopy,
            .data_len = dataLen,
            .width = (uint32_t)width,
            .height = (uint32_t)height,
            .pixel_format = "BGRA",
            .timestamp_us = timestamp_us,
            .stride = (uint32_t)stride,
        };

        self.callbackContext->frame_callback(self.callbackContext->rust_context, frame);
        free(dataCopy);
    }

    CVPixelBufferUnlockBaseAddress(imageBuffer, kCVPixelBufferLock_ReadOnly);
    self.frameCount++;
}

- (void)dealloc {
    [self stop];
}

@end

// ---- C API (called from Rust) ----

extern "C" {

/// Start webcam capture with preview. Returns opaque session pointer or NULL.
/// This starts the AVCaptureSession and shows a preview window.
/// Frame callback is NOT active yet — call avc_start_recording() to begin encoding.
void* avc_start_webcam(
    const char* device_id,
    int fps,
    int width,
    int height,
    void* rust_context,
    void (*frame_callback)(void* context, AVCFrameData frame)
) {
    // rust_context and frame_callback are stored but not used until avc_start_recording
    (void)rust_context;
    (void)frame_callback;
    (void)width;
    (void)height;

    @autoreleasepool {
        NSString* devId = device_id ? [NSString stringWithUTF8String:device_id] : nil;

        AVCCameraSession* session = [[AVCCameraSession alloc] init];
        BOOL ok = [session startWithDeviceId:devId fps:fps];
        if (!ok) {
            return NULL;
        }

        // Show the preview window immediately
        [session showPreview];

        return (__bridge_retained void*)session;
    }
}

/// Begin recording frames (start sending to Rust callback).
/// Call this when screen recording starts.
void avc_start_recording(
    void* session_ptr,
    void* rust_context,
    void (*frame_callback)(void* context, AVCFrameData frame)
) {
    if (!session_ptr) return;
    @autoreleasepool {
        AVCCameraSession* session = (__bridge AVCCameraSession*)session_ptr;

        AVCCallbackContext* ctx = (AVCCallbackContext*)malloc(sizeof(AVCCallbackContext));
        ctx->rust_context = rust_context;
        ctx->frame_callback = frame_callback;

        // Hide preview during recording (so it doesn't appear in screen capture)
        [session hidePreview];
        [session startRecordingWithCallback:ctx];
    }
}

/// Stop recording frames (stop sending to Rust callback).
/// The capture session keeps running for potential future use.
void avc_stop_recording(void* session_ptr) {
    if (!session_ptr) return;
    @autoreleasepool {
        AVCCameraSession* session = (__bridge AVCCameraSession*)session_ptr;
        AVCCallbackContext* ctx = session.callbackContext;
        [session stopRecording];
        session.callbackContext = NULL;
        if (ctx) free(ctx);
    }
}

/// Stop webcam capture entirely and clean up.
void avc_stop_webcam(void* session_ptr) {
    if (!session_ptr) return;
    @autoreleasepool {
        AVCCameraSession* session = (__bridge_transfer AVCCameraSession*)session_ptr;
        AVCCallbackContext* ctx = session.callbackContext;
        [session stop];
        session.callbackContext = NULL;
        if (ctx) free(ctx);
    }
}

/// Check if camera permission is granted.
bool avc_check_camera_permission(void) {
    AVAuthorizationStatus status = [AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeVideo];
    return status == AVAuthorizationStatusAuthorized;
}

/// Request camera permission. Blocks until user responds.
bool avc_request_camera_permission(void) {
    __block BOOL granted = NO;
    dispatch_semaphore_t sem = dispatch_semaphore_create(0);
    [AVCaptureDevice requestAccessForMediaType:AVMediaTypeVideo completionHandler:^(BOOL g) {
        granted = g;
        dispatch_semaphore_signal(sem);
    }];
    dispatch_semaphore_wait(sem, dispatch_time(DISPATCH_TIME_NOW, 10 * NSEC_PER_SEC));
    return granted;
}

} // extern "C"
