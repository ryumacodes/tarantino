// Objective-C++ wrapper for ScreenCaptureKit
//
// This file provides a C-compatible interface to ScreenCaptureKit APIs
// for use from Rust via FFI.

#import <Foundation/Foundation.h>
#import <ScreenCaptureKit/ScreenCaptureKit.h>
#import <CoreMedia/CoreMedia.h>
#import <CoreVideo/CoreVideo.h>
#import <CoreGraphics/CoreGraphics.h>

#include <stdint.h>
#include <stdbool.h>

// C-compatible structures matching the Rust FFI definitions

typedef struct {
    uint64_t display_id;
    const char* name;
    uint32_t width;
    uint32_t height;
    double scale_factor;
    bool is_primary;
} SCKDisplay;

typedef struct {
    uint64_t window_id;
    const char* title;
    uint32_t width;
    uint32_t height;
    const char* owner_name;
} SCKWindow;

typedef struct {
    bool screen_recording;
    bool microphone;
    bool camera;
} SCKPermissionStatus;

typedef struct {
    uint64_t source_id;
    bool is_display;
    uint32_t fps;
    bool include_cursor;
    bool include_audio;
    uint32_t crop_x;
    uint32_t crop_y;
    uint32_t crop_width;
    uint32_t crop_height;
    const char* output_path; // optional; unused in current flow
} SCKCaptureConfig;

typedef struct {
    const uint8_t* data;
    size_t data_len;
    uint32_t width;
    uint32_t height;
    const char* pixel_format;
    uint64_t timestamp_us;
    uint32_t stride;
} SCKFrameData;

typedef struct {
    const uint8_t* data;
    size_t data_len;
    uint32_t sample_rate;
    uint32_t channels;
    uint64_t timestamp_us;
} SCKAudioData;

// Frame callback function pointer type
typedef void (*SCKFrameCallbackFn)(void* context, SCKFrameData frame);

// Audio callback function pointer type
typedef void (*SCKAudioCallbackFn)(void* context, SCKAudioData audio);

// Context structure to hold the callbacks
typedef struct {
    void* rust_context;
    SCKFrameCallbackFn frame_callback;
    SCKAudioCallbackFn audio_callback;
} SCKCallbackContext;

// Forward declaration of capture session class
@interface SCKCaptureSession : NSObject <SCStreamOutput, SCStreamDelegate>
@property (nonatomic, strong) SCStream* stream;
@property (nonatomic, strong) SCContentFilter* filter;
@property (nonatomic, strong) SCStreamConfiguration* config;
@property (nonatomic, assign) SCKCallbackContext* callbackContext;
@property (nonatomic, assign) BOOL isCapturing;

// Frame drop statistics
@property (nonatomic, assign) uint64_t totalFramesReceived;
@property (nonatomic, assign) uint64_t framesDroppedNoBuffer;
@property (nonatomic, assign) uint64_t lastStatsLogTime;
@end

@implementation SCKCaptureSession

- (void)stream:(SCStream*)stream
    didOutputSampleBuffer:(CMSampleBufferRef)sampleBuffer
    ofType:(SCStreamOutputType)type {

    if (!self.isCapturing || !self.callbackContext) {
        return;
    }

    @autoreleasepool {
        if (type == SCStreamOutputTypeScreen) {
            // Handle video frames
            if (!self.callbackContext->frame_callback) {
                return;
            }

            self.totalFramesReceived++;

            // Get image buffer from sample buffer
            CVImageBufferRef imageBuffer = CMSampleBufferGetImageBuffer(sampleBuffer);
            if (!imageBuffer) {
                // Track dropped frames due to missing image buffer
                self.framesDroppedNoBuffer++;

                // Calculate drop rate
                double dropRate = (double)self.framesDroppedNoBuffer / (double)self.totalFramesReceived;

                // Only log if drop rate exceeds 5% threshold (indicates a real problem)
                // Occasional drops are normal under system pressure
                if (dropRate > 0.05) {
                    // Log statistics every 5 seconds to avoid spam
                    uint64_t now = (uint64_t)[[NSDate date] timeIntervalSince1970];
                    if (now - self.lastStatsLogTime >= 5) {
                        NSLog(@"[SCK WARNING] High frame drop rate: %llu/%llu frames (%.1f%%) missing image buffer",
                              self.framesDroppedNoBuffer, self.totalFramesReceived, dropRate * 100.0);
                        NSLog(@"[SCK WARNING] This may indicate system memory pressure or competing screen capture apps");
                        self.lastStatsLogTime = now;
                    }
                }
                return;
            }

            // Lock the base address
            CVPixelBufferLockBaseAddress(imageBuffer, kCVPixelBufferLock_ReadOnly);

            // Get buffer information
            size_t width = CVPixelBufferGetWidth(imageBuffer);
            size_t height = CVPixelBufferGetHeight(imageBuffer);
            size_t bytesPerRow = CVPixelBufferGetBytesPerRow(imageBuffer);
            void* baseAddress = CVPixelBufferGetBaseAddress(imageBuffer);

            // Get pixel format
            OSType pixelFormat = CVPixelBufferGetPixelFormatType(imageBuffer);
            const char* formatStr = "BGRA"; // Default

            if (pixelFormat == kCVPixelFormatType_32BGRA) {
                formatStr = "BGRA";
            } else if (pixelFormat == kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange) {
                formatStr = "NV12";
            }

            // Get timestamp
            CMTime presentationTime = CMSampleBufferGetPresentationTimeStamp(sampleBuffer);
            uint64_t timestamp_us = (uint64_t)((presentationTime.value * 1000000) / presentationTime.timescale);

            // Create frame data
            size_t dataSize = bytesPerRow * height;

            SCKFrameData frame = {
                .data = (const uint8_t*)baseAddress,
                .data_len = dataSize,
                .width = (uint32_t)width,
                .height = (uint32_t)height,
                .pixel_format = formatStr,
                .timestamp_us = timestamp_us,
                .stride = (uint32_t)bytesPerRow
            };

            // Call frame callback through context
            self.callbackContext->frame_callback(self.callbackContext->rust_context, frame);

            // Unlock the base address
            CVPixelBufferUnlockBaseAddress(imageBuffer, kCVPixelBufferLock_ReadOnly);
        }
#if __MAC_OS_X_VERSION_MAX_ALLOWED >= 130000
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wunguarded-availability"
        else if (type == SCStreamOutputTypeAudio) {
                // Handle audio samples
                if (!self.callbackContext->audio_callback) {
                    return;
                }

                // Get audio buffer list from sample buffer
                CMBlockBufferRef blockBuffer = CMSampleBufferGetDataBuffer(sampleBuffer);
                if (!blockBuffer) {
                    return;
                }

                // Get audio format description
                CMFormatDescriptionRef formatDesc = CMSampleBufferGetFormatDescription(sampleBuffer);
                const AudioStreamBasicDescription* audioDesc = CMAudioFormatDescriptionGetStreamBasicDescription(formatDesc);

                if (!audioDesc) {
                    return;
                }

                // Get audio data pointer and size
                char* dataPointer = NULL;
                size_t dataLength = 0;

                OSStatus status = CMBlockBufferGetDataPointer(blockBuffer, 0, NULL, &dataLength, &dataPointer);
                if (status != kCMBlockBufferNoErr || !dataPointer) {
                    return;
                }

                // Get timestamp
                CMTime presentationTime = CMSampleBufferGetPresentationTimeStamp(sampleBuffer);
                uint64_t timestamp_us = (uint64_t)((presentationTime.value * 1000000) / presentationTime.timescale);

                // Create audio data
                SCKAudioData audio = {
                    .data = (const uint8_t*)dataPointer,
                    .data_len = dataLength,
                    .sample_rate = (uint32_t)audioDesc->mSampleRate,
                    .channels = audioDesc->mChannelsPerFrame,
                    .timestamp_us = timestamp_us
                };

                // Call audio callback through context
                self.callbackContext->audio_callback(self.callbackContext->rust_context, audio);
        }
#pragma clang diagnostic pop
#endif
    }
}

- (void)stream:(SCStream*)stream didStopWithError:(NSError*)error {
    if (error) {
        NSLog(@"Stream stopped with error: %@", error);
    }
    self.isCapturing = NO;
}

@end

// C API implementation

extern "C" {

// Check if ScreenCaptureKit is available
bool sck_is_available() {
    if (@available(macOS 12.3, *)) {
        return true;
    }
    return false;
}

// Get shareable displays
bool sck_get_shareable_displays(SCKDisplay** out_displays, size_t* out_count) {
    if (@available(macOS 12.3, *)) {
        __block bool success = false;
        __block NSArray<SCDisplay*>* displays = nil;

        dispatch_semaphore_t semaphore = dispatch_semaphore_create(0);

        [SCShareableContent getShareableContentExcludingDesktopWindows:YES
                                                    onScreenWindowsOnly:YES
                                                      completionHandler:^(SCShareableContent* content, NSError* error) {
            if (error) {
                NSLog(@"Failed to get shareable content: %@", error);
            } else {
                displays = content.displays;
                success = true;
            }
            dispatch_semaphore_signal(semaphore);
        }];

        dispatch_semaphore_wait(semaphore, DISPATCH_TIME_FOREVER);

        if (!success || !displays) {
            *out_count = 0;
            return false;
        }

        *out_count = displays.count;
        *out_displays = (SCKDisplay*)malloc(sizeof(SCKDisplay) * displays.count);

        for (size_t i = 0; i < displays.count; i++) {
            SCDisplay* display = displays[i];
            CGRect frame = display.frame;

            // Get display name - use displayID as fallback
            NSString* displayName = [NSString stringWithFormat:@"Display %u", display.displayID];

            // Get actual scale factor from display mode
            // This compares pixel (physical) dimensions to logical (point) dimensions
            double scale_factor = 1.0;
            CGDisplayModeRef mode = CGDisplayCopyDisplayMode(display.displayID);
            if (mode) {
                size_t pixelWidth = CGDisplayModeGetPixelWidth(mode);
                size_t logicalWidth = CGDisplayModeGetWidth(mode);
                if (logicalWidth > 0) {
                    scale_factor = (double)pixelWidth / (double)logicalWidth;
                }
                CGDisplayModeRelease(mode);
            }

            (*out_displays)[i] = (SCKDisplay){
                .display_id = display.displayID,
                .name = strdup([displayName UTF8String]),
                .width = (uint32_t)frame.size.width,
                .height = (uint32_t)frame.size.height,
                .scale_factor = scale_factor,
                .is_primary = (i == 0) // First display is usually primary
            };
        }

        return true;
    }

    *out_count = 0;
    return false;
}

// Get shareable windows
bool sck_get_shareable_windows(SCKWindow** out_windows, size_t* out_count) {
    if (@available(macOS 12.3, *)) {
        __block bool success = false;
        __block NSArray<SCWindow*>* windows = nil;

        dispatch_semaphore_t semaphore = dispatch_semaphore_create(0);

        [SCShareableContent getShareableContentExcludingDesktopWindows:NO
                                                    onScreenWindowsOnly:YES
                                                      completionHandler:^(SCShareableContent* content, NSError* error) {
            if (error) {
                NSLog(@"Failed to get shareable content: %@", error);
            } else {
                windows = content.windows;
                success = true;
            }
            dispatch_semaphore_signal(semaphore);
        }];

        dispatch_semaphore_wait(semaphore, DISPATCH_TIME_FOREVER);

        if (!success || !windows) {
            *out_count = 0;
            return false;
        }

        *out_count = windows.count;
        *out_windows = (SCKWindow*)malloc(sizeof(SCKWindow) * windows.count);

        for (size_t i = 0; i < windows.count; i++) {
            SCWindow* window = windows[i];
            CGRect frame = window.frame;

            (*out_windows)[i] = (SCKWindow){
                .window_id = window.windowID,
                .title = strdup([window.title UTF8String] ?: "Untitled"),
                .width = (uint32_t)frame.size.width,
                .height = (uint32_t)frame.size.height,
                .owner_name = strdup([window.owningApplication.applicationName UTF8String] ?: "Unknown")
            };
        }

        return true;
    }

    *out_count = 0;
    return false;
}

// Check permissions
SCKPermissionStatus sck_check_permissions() {
    SCKPermissionStatus status = {false, false, false};

    // Check screen recording permission
    if (@available(macOS 10.15, *)) {
        // Use CGPreflightScreenCaptureAccess to check without prompting
        status.screen_recording = CGPreflightScreenCaptureAccess();
    }

    // TODO: Check microphone and camera permissions
    // For now, assume not granted

    return status;
}

// Request permissions
SCKPermissionStatus sck_request_permissions() {
    SCKPermissionStatus status = {false, false, false};

    // Request screen recording permission
    if (@available(macOS 10.15, *)) {
        // CGRequestScreenCaptureAccess triggers the system prompt if needed
        status.screen_recording = CGRequestScreenCaptureAccess();
    }

    // TODO: Request microphone and camera permissions if needed

    return status;
}

// Free display array
void sck_free_displays(SCKDisplay* displays, size_t count) {
    if (!displays) return;

    for (size_t i = 0; i < count; i++) {
        if (displays[i].name) {
            free((void*)displays[i].name);
        }
    }

    free(displays);
}

// Free window array
void sck_free_windows(SCKWindow* windows, size_t count) {
    if (!windows) return;

    for (size_t i = 0; i < count; i++) {
        if (windows[i].title) {
            free((void*)windows[i].title);
        }
        if (windows[i].owner_name) {
            free((void*)windows[i].owner_name);
        }
    }

    free(windows);
}

// Start capture
void* sck_start_capture(SCKCaptureConfig config, void* rust_context, SCKFrameCallbackFn frame_callback, SCKAudioCallbackFn audio_callback) {
    if (@available(macOS 12.3, *)) {
        __block SCKCaptureSession* session = nil;
        __block bool success = false;

        dispatch_semaphore_t semaphore = dispatch_semaphore_create(0);

        // Get shareable content
        [SCShareableContent getShareableContentExcludingDesktopWindows:YES
                                                    onScreenWindowsOnly:YES
                                                      completionHandler:^(SCShareableContent* content, NSError* error) {
            if (error) {
                NSLog(@"Failed to get shareable content: %@", error);
                dispatch_semaphore_signal(semaphore);
                return;
            }

            // Find the target display or window
            SCContentFilter* filter = nil;
            SCDisplay* targetDisplay = nil;  // Keep reference for stream configuration

            if (config.is_display) {
                // Find display by ID
                for (SCDisplay* display in content.displays) {
                    if (display.displayID == config.source_id) {
                        targetDisplay = display;
                        break;
                    }
                }

                if (!targetDisplay) {
                    NSLog(@"Display not found: %llu", config.source_id);
                    dispatch_semaphore_signal(semaphore);
                    return;
                }

                filter = [[SCContentFilter alloc] initWithDisplay:targetDisplay
                                                  excludingWindows:@[]];
            } else {
                // Find window by ID
                SCWindow* targetWindow = nil;
                for (SCWindow* window in content.windows) {
                    if (window.windowID == config.source_id) {
                        targetWindow = window;
                        break;
                    }
                }

                if (!targetWindow) {
                    NSLog(@"Window not found: %llu", config.source_id);
                    dispatch_semaphore_signal(semaphore);
                    return;
                }

                filter = [[SCContentFilter alloc] initWithDesktopIndependentWindow:targetWindow];
            }

            // Configure stream - use display dimensions if no crop specified
            SCStreamConfiguration* streamConfig = [[SCStreamConfiguration alloc] init];
            if (config.crop_width > 0 && config.crop_height > 0) {
                streamConfig.width = config.crop_width;
                streamConfig.height = config.crop_height;
            } else if (config.is_display && targetDisplay) {
                // Use physical pixel dimensions (not logical points) for Retina displays
                CGRect displayFrame = targetDisplay.frame;
                double scale_factor = 1.0;
                CGDisplayModeRef mode = CGDisplayCopyDisplayMode(targetDisplay.displayID);
                if (mode) {
                    size_t pixelWidth = CGDisplayModeGetPixelWidth(mode);
                    size_t logicalWidth = CGDisplayModeGetWidth(mode);
                    if (logicalWidth > 0) {
                        scale_factor = (double)pixelWidth / (double)logicalWidth;
                    }
                    CGDisplayModeRelease(mode);
                }
                streamConfig.width = (size_t)(displayFrame.size.width * scale_factor);
                streamConfig.height = (size_t)(displayFrame.size.height * scale_factor);
                NSLog(@"[SCK] Using display pixel dimensions: %zux%zu (scale: %.1f)",
                      (size_t)(displayFrame.size.width * scale_factor),
                      (size_t)(displayFrame.size.height * scale_factor), scale_factor);
            } else {
                // Fallback for windows or unknown cases
                streamConfig.width = 1920;
                streamConfig.height = 1080;
            }
            streamConfig.minimumFrameInterval = CMTimeMake(1, config.fps);
            streamConfig.pixelFormat = kCVPixelFormatType_32BGRA;
            // Always hide system cursor - we render our own overlay cursor in the editor
            // This allows cursor settings (style, hide, rotation, click effects) to work
            streamConfig.showsCursor = NO;
            streamConfig.scalesToFit = YES;

            // Increase queue depth to reduce frame drops under memory pressure
            // Default is typically 3-5; setting to 12 provides more buffer headroom
            // Note: Frame drops in dev builds are expected due to unoptimized code
            // Release builds should have significantly better performance
            streamConfig.queueDepth = 12;

            // Enable audio capture if requested (macOS 13+)
#if __MAC_OS_X_VERSION_MAX_ALLOWED >= 130000
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wunguarded-availability"
            if (config.include_audio) {
                streamConfig.capturesAudio = YES;
            }
#pragma clang diagnostic pop
#endif

            // Create callback context
            SCKCallbackContext* ctx = (SCKCallbackContext*)malloc(sizeof(SCKCallbackContext));
            ctx->rust_context = rust_context;
            ctx->frame_callback = frame_callback;
            ctx->audio_callback = audio_callback;

            // Create capture session
            session = [[SCKCaptureSession alloc] init];
            session.filter = filter;
            session.config = streamConfig;
            session.callbackContext = ctx;

            // Initialize frame statistics
            session.totalFramesReceived = 0;
            session.framesDroppedNoBuffer = 0;
            session.lastStatsLogTime = 0;

            // Create stream
            NSError* streamError = nil;
            session.stream = [[SCStream alloc] initWithFilter:filter
                                                 configuration:streamConfig
                                                      delegate:session];

            if (streamError) {
                NSLog(@"Failed to create stream: %@", streamError);
                dispatch_semaphore_signal(semaphore);
                return;
            }

            // Create dedicated serial queue with USER_INTERACTIVE QoS for frame handling
            // This reduces frame drops by ensuring consistent, high-priority processing
            dispatch_queue_attr_t frameQueueAttr = dispatch_queue_attr_make_with_qos_class(
                DISPATCH_QUEUE_SERIAL, QOS_CLASS_USER_INTERACTIVE, 0);
            dispatch_queue_t frameQueue = dispatch_queue_create("com.tarantino.frameQueue", frameQueueAttr);

            // Add stream output for video
            NSError* outputError = nil;
            [session.stream addStreamOutput:session
                                       type:SCStreamOutputTypeScreen
                         sampleHandlerQueue:frameQueue
                                      error:&outputError];

            if (outputError) {
                NSLog(@"Failed to add video stream output: %@", outputError);
                dispatch_semaphore_signal(semaphore);
                return;
            }

            // Add stream output for audio if enabled (macOS 13+)
#if __MAC_OS_X_VERSION_MAX_ALLOWED >= 130000
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wunguarded-availability"
            if (config.include_audio) {
                // Create dedicated queue for audio as well
                dispatch_queue_attr_t audioQueueAttr = dispatch_queue_attr_make_with_qos_class(
                    DISPATCH_QUEUE_SERIAL, QOS_CLASS_USER_INTERACTIVE, 0);
                dispatch_queue_t audioQueue = dispatch_queue_create("com.tarantino.audioQueue", audioQueueAttr);

                NSError* audioOutputError = nil;
                [session.stream addStreamOutput:session
                                           type:SCStreamOutputTypeAudio
                             sampleHandlerQueue:audioQueue
                                          error:&audioOutputError];

                if (audioOutputError) {
                    NSLog(@"Failed to add audio stream output: %@", audioOutputError);
                    // Continue anyway - video capture will still work
                }
            }
#pragma clang diagnostic pop
#endif

            // Start capture
            [session.stream startCaptureWithCompletionHandler:^(NSError* error) {
                if (error) {
                    NSLog(@"Failed to start capture: %@", error);
                } else {
                    session.isCapturing = YES;
                    success = true;
                }
                dispatch_semaphore_signal(semaphore);
            }];
        }];

        dispatch_semaphore_wait(semaphore, DISPATCH_TIME_FOREVER);

        if (success && session) {
            return (__bridge_retained void*)session;
        }
    }

    return nullptr;
}

// Stop capture
bool sck_stop_capture(void* instance, void** out_rust_context) {
    if (!instance) {
        return false;
    }

    if (@available(macOS 12.3, *)) {
        SCKCaptureSession* session = (__bridge_transfer SCKCaptureSession*)instance;

        if (session.stream && session.isCapturing) {
            __block bool stopped = false;
            dispatch_semaphore_t semaphore = dispatch_semaphore_create(0);

            [session.stream stopCaptureWithCompletionHandler:^(NSError* error) {
                if (error) {
                    NSLog(@"Failed to stop capture: %@", error);
                } else {
                    stopped = true;
                }
                dispatch_semaphore_signal(semaphore);
            }];

            dispatch_semaphore_wait(semaphore, DISPATCH_TIME_FOREVER);

            session.isCapturing = NO;

            // Log final statistics
            if (session.totalFramesReceived > 0) {
                double dropRate = (double)session.framesDroppedNoBuffer / (double)session.totalFramesReceived;
                NSLog(@"[SCK STATS] Capture session ended - Total frames: %llu, Dropped: %llu (%.2f%%)",
                      session.totalFramesReceived, session.framesDroppedNoBuffer, dropRate * 100.0);
            }

            session.stream = nil;

            // Extract rust_context and return it to Rust for proper cleanup
            if (session.callbackContext) {
                if (out_rust_context) {
                    *out_rust_context = session.callbackContext->rust_context;
                }
                // Free the C callback context struct (but not the rust_context inside it)
                free(session.callbackContext);
                session.callbackContext = nullptr;
            } else if (out_rust_context) {
                *out_rust_context = nullptr;
            }

            return stopped;
        }
    }

    return false;
}

} // extern "C"
