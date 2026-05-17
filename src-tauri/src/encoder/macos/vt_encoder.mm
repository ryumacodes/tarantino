// Objective-C++ wrapper for VideoToolbox encoding
//
// This file provides a C-compatible interface to VideoToolbox compression
// for use from Rust via FFI.

#import <Foundation/Foundation.h>
#import <VideoToolbox/VideoToolbox.h>
#import <CoreVideo/CoreVideo.h>
#import <CoreMedia/CoreMedia.h>
#import <CoreFoundation/CoreFoundation.h>

#include <stdint.h>
#include <stdbool.h>
#include <stdlib.h>

// C-compatible structures matching the Rust FFI definitions

typedef struct {
    uint32_t width;
    uint32_t height;
    uint32_t fps;
    uint32_t bitrate;        // in bps
    uint32_t keyframe_interval; // in frames
    uint32_t quality;        // 0-100
    bool enable_realtime;    // Low-latency encoding
    const char* profile;     // "baseline", "main", "high"
} VTEncoderConfig;

typedef struct {
    const uint8_t* data;
    size_t data_len;
    uint64_t timestamp_us;
    bool is_keyframe;
    int64_t pts;
    int64_t dts;
    const uint8_t* sps_data;
    size_t sps_len;
    const uint8_t* pps_data;
    size_t pps_len;
} VTEncodedFrame;

// Frame callback function pointer type
typedef void (*VTFrameCallbackFn)(void* context, VTEncodedFrame frame);

// Context structure to hold the callback
typedef struct {
    void* rust_context;
    VTFrameCallbackFn callback;
} VTCallbackContext;

// Encoder session wrapper
#include "vt_encoder_session.inc.mm"

extern "C" {

// Check if VideoToolbox is available
bool vt_encoder_is_available() {
    return true; // VideoToolbox is available on all modern macOS versions
}

// Create and configure VideoToolbox encoder
void* vt_encoder_create(VTEncoderConfig config, void* rust_context, VTFrameCallbackFn callback) {
    VTEncoderSession* session = [[VTEncoderSession alloc] init];
    session.width = config.width;
    session.height = config.height;
    session.fps = config.fps;
    session.frameCount = 0;
    // Use high timescale for sub-frame precision (matches MP4 muxer expectation)
    session.timeScale = 60000; // 60 fps * 1000 for millisecond precision

    // Create callback context
    VTCallbackContext* ctx = (VTCallbackContext*)malloc(sizeof(VTCallbackContext));
    ctx->rust_context = rust_context;
    ctx->callback = callback;
    session.callbackContext = ctx;

    // Create source image buffer attributes for better pixel format handling
    NSDictionary* sourceImageBufferAttributes = @{
        (NSString*)kCVPixelBufferPixelFormatTypeKey: @(kCVPixelFormatType_32BGRA),
        (NSString*)kCVPixelBufferWidthKey: @(config.width),
        (NSString*)kCVPixelBufferHeightKey: @(config.height),
        (NSString*)kCVPixelBufferIOSurfacePropertiesKey: @{},
        (NSString*)kCVPixelBufferMetalCompatibilityKey: @YES,
    };

    // Create compression session
    VTCompressionSessionRef compressionSession = NULL;
    OSStatus status = VTCompressionSessionCreate(
        kCFAllocatorDefault,
        config.width,
        config.height,
        kCMVideoCodecType_H264,
        NULL, // encoder spec (NULL = default, uses hardware if available)
        (__bridge CFDictionaryRef)sourceImageBufferAttributes,
        kCFAllocatorDefault,
        vtCompressionOutputCallback,
        (__bridge void*)session,
        &compressionSession
    );

    if (status != noErr) {
        NSLog(@"Failed to create VTCompressionSession: %d", (int)status);
        return nullptr;
    }

    session.compressionSession = compressionSession;

    // Configure encoder properties

    // Set realtime encoding
    if (config.enable_realtime) {
        VTSessionSetProperty(session.compressionSession,
                           kVTCompressionPropertyKey_RealTime,
                           kCFBooleanTrue);
    }

    // Set profile level
    CFStringRef profileLevel = kVTProfileLevel_H264_Main_AutoLevel;
    if (config.profile) {
        NSString* profileStr = [NSString stringWithUTF8String:config.profile];
        if ([profileStr isEqualToString:@"baseline"]) {
            profileLevel = kVTProfileLevel_H264_Baseline_AutoLevel;
        } else if ([profileStr isEqualToString:@"high"]) {
            profileLevel = kVTProfileLevel_H264_High_AutoLevel;
        }
    }
    VTSessionSetProperty(session.compressionSession,
                       kVTCompressionPropertyKey_ProfileLevel,
                       profileLevel);

    // Set H.264 entropy mode to CABAC for better compression and error resilience
    // CABAC provides better compression and more robust error recovery than CAVLC
    // Modern decoders (including VideoToolbox itself) fully support CABAC
    CFStringRef entropyMode = kVTH264EntropyMode_CABAC;
    VTSessionSetProperty(session.compressionSession,
                       kVTCompressionPropertyKey_H264EntropyMode,
                       entropyMode);

    // Set average bitrate (use VBR for better quality and fewer encoding errors)
    if (config.bitrate > 0) {
        CFNumberRef bitrateNumber = CFNumberCreate(kCFAllocatorDefault, kCFNumberSInt32Type, &config.bitrate);
        VTSessionSetProperty(session.compressionSession,
                           kVTCompressionPropertyKey_AverageBitRate,
                           bitrateNumber);
        CFRelease(bitrateNumber);

        // Configure VBR with generous data rate limits to allow bitrate spikes
        // This prevents encoder from dropping/corrupting frames when scenes are complex
        // Set max to 2x average bitrate to allow bursts while maintaining quality
        int32_t maxBitrate = config.bitrate * 2;
        CFNumberRef maxBitrateNumber = CFNumberCreate(kCFAllocatorDefault, kCFNumberSInt32Type, &maxBitrate);
        VTSessionSetProperty(session.compressionSession,
                           kVTCompressionPropertyKey_DataRateLimits,
                           maxBitrateNumber);
        CFRelease(maxBitrateNumber);

        NSLog(@"[VT CONFIG] Configured VBR: average=%d bps, max=%d bps", config.bitrate, maxBitrate);
    }

    // Set keyframe interval (GOP size)
    if (config.keyframe_interval > 0) {
        CFNumberRef intervalNumber = CFNumberCreate(kCFAllocatorDefault, kCFNumberSInt32Type, &config.keyframe_interval);
        VTSessionSetProperty(session.compressionSession,
                           kVTCompressionPropertyKey_MaxKeyFrameInterval,
                           intervalNumber);
        CFRelease(intervalNumber);
    }

    // Set expected frame rate
    CFNumberRef fpsNumber = CFNumberCreate(kCFAllocatorDefault, kCFNumberSInt32Type, &config.fps);
    VTSessionSetProperty(session.compressionSession,
                       kVTCompressionPropertyKey_ExpectedFrameRate,
                       fpsNumber);
    CFRelease(fpsNumber);

    // Set quality (0.0 - 1.0, where 1.0 is highest quality)
    // Quality parameter works best with VBR mode to maintain consistent visual quality
    // Higher quality = encoder will use more bitrate to preserve detail
    if (config.quality > 0) {
        double qualityValue = config.quality / 100.0;
        CFNumberRef qualityNumber = CFNumberCreate(kCFAllocatorDefault, kCFNumberDoubleType, &qualityValue);
        VTSessionSetProperty(session.compressionSession,
                           kVTCompressionPropertyKey_Quality,
                           qualityNumber);
        CFRelease(qualityNumber);
        NSLog(@"[VT CONFIG] Quality set to %.2f (VBR mode)", qualityValue);
    } else {
        // If no quality specified, use a sensible default (0.75 = high quality)
        // This prevents encoder from being too aggressive with compression
        double defaultQuality = 0.75;
        CFNumberRef qualityNumber = CFNumberCreate(kCFAllocatorDefault, kCFNumberDoubleType, &defaultQuality);
        VTSessionSetProperty(session.compressionSession,
                           kVTCompressionPropertyKey_Quality,
                           qualityNumber);
        CFRelease(qualityNumber);
        NSLog(@"[VT CONFIG] Using default quality: %.2f", defaultQuality);
    }

    // Disable frame reordering for stability and simpler timestamp handling
    // This ensures PTS == DTS, avoiding complex B-frame timing issues
    VTSessionSetProperty(session.compressionSession,
                       kVTCompressionPropertyKey_AllowFrameReordering,
                       kCFBooleanFalse);

    // Set max frame delay count to limit encoder buffering
    // This helps prevent frame queue buildup and reduces encoding latency
    // Lower values = lower latency but potentially lower compression efficiency
    int32_t maxFrameDelayCount = 3; // Allow up to 3 frames in encoder buffer
    CFNumberRef maxDelayNumber = CFNumberCreate(kCFAllocatorDefault, kCFNumberSInt32Type, &maxFrameDelayCount);
    VTSessionSetProperty(session.compressionSession,
                       kVTCompressionPropertyKey_MaxFrameDelayCount,
                       maxDelayNumber);
    CFRelease(maxDelayNumber);
    NSLog(@"[VT CONFIG] Max frame delay count: %d frames", maxFrameDelayCount);

    // NOTE: VideoToolbox outputs H.264 in AVCC format (4-byte length prefixes) by default.
    // This is the correct format for MP4 muxing. The muxer should handle AVCC input directly
    // without attempting to convert from Annex B (which has start codes).

    // Prepare to encode
    status = VTCompressionSessionPrepareToEncodeFrames(session.compressionSession);
    if (status != noErr) {
        NSLog(@"Failed to prepare compression session: %d", (int)status);
        return nullptr;
    }

    NSLog(@"VideoToolbox encoder created: %dx%d @ %d fps, bitrate: %d bps",
          config.width, config.height, config.fps, config.bitrate);

    return (__bridge_retained void*)session;
}

// Encode a frame
bool vt_encoder_encode_frame(
    void* instance,
    const uint8_t* frame_data,
    size_t data_len,
    uint32_t width,
    uint32_t height,
    uint32_t stride,
    const char* pixel_format,
    uint64_t timestamp_us
) {
    (void)data_len; // Unused parameter (frame size calculated from width * height * 4)

    if (!instance) {
        return false;
    }

    VTEncoderSession* session = (__bridge VTEncoderSession*)instance;

    @autoreleasepool {
        // Create pixel buffer from frame data
        CVPixelBufferRef pixelBuffer = NULL;

        // Determine pixel format
        OSType formatType = kCVPixelFormatType_32BGRA; // Default
        if (pixel_format && strcmp(pixel_format, "NV12") == 0) {
            formatType = kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange;
        }

        // Create pixel buffer with proper memory management
        // FIX: Previously used CVPixelBufferCreateWithBytes with NULL callback,
        // which caused memory corruption. Now we properly allocate and copy.
        NSDictionary* pixelBufferAttributes = @{
            (NSString*)kCVPixelBufferIOSurfacePropertiesKey: @{},
            (NSString*)kCVPixelBufferMetalCompatibilityKey: @YES,
        };

        CVReturn cvStatus = CVPixelBufferCreate(
            kCFAllocatorDefault,
            width,
            height,
            formatType,
            (__bridge CFDictionaryRef)pixelBufferAttributes,
            &pixelBuffer
        );

        if (cvStatus != kCVReturnSuccess || !pixelBuffer) {
            NSLog(@"Failed to create pixel buffer: %d", cvStatus);
            return false;
        }

        // Lock the pixel buffer and copy data into it
        cvStatus = CVPixelBufferLockBaseAddress(pixelBuffer, 0);
        if (cvStatus != kCVReturnSuccess) {
            NSLog(@"Failed to lock pixel buffer: %d", cvStatus);
            CVPixelBufferRelease(pixelBuffer);
            return false;
        }

        // Copy frame data into pixel buffer
        void* baseAddress = CVPixelBufferGetBaseAddress(pixelBuffer);
        size_t bytesPerRow = CVPixelBufferGetBytesPerRow(pixelBuffer);

        if (formatType == kCVPixelFormatType_32BGRA) {
            // Copy BGRA data row by row
            for (uint32_t row = 0; row < height; row++) {
                memcpy(
                    (uint8_t*)baseAddress + (row * bytesPerRow),
                    frame_data + (row * stride),
                    width * 4 // 4 bytes per pixel for BGRA
                );
            }
        } else {
            // For NV12 or other formats, copy directly
            size_t copySize = height * stride;
            memcpy(baseAddress, frame_data, copySize);
        }

        CVPixelBufferUnlockBaseAddress(pixelBuffer, 0);

        // Create presentation timestamp with corrected timescale
        // Calculate timestamp in timescale units (60000 = 60fps * 1000ms precision)
        int64_t timestamp_in_timescale = (timestamp_us * session.timeScale) / 1000000;
        CMTime presentationTime = CMTimeMake(timestamp_in_timescale, session.timeScale);

        // Encode the frame
        VTEncodeInfoFlags infoFlags = 0;
        OSStatus status = VTCompressionSessionEncodeFrame(
            session.compressionSession,
            pixelBuffer,
            presentationTime,
            kCMTimeInvalid, // duration (invalid = unknown)
            NULL, // frame properties
            NULL, // source frame reference
            &infoFlags
        );

        CVPixelBufferRelease(pixelBuffer);

        if (status != noErr) {
            NSLog(@"VTCompressionSessionEncodeFrame failed: %d", (int)status);
            return false;
        }

        session.frameCount++;
        return true;
    }
}

// Flush pending frames
bool vt_encoder_flush(void* instance) {
    if (!instance) {
        return false;
    }

    VTEncoderSession* session = (__bridge VTEncoderSession*)instance;

    OSStatus status = VTCompressionSessionCompleteFrames(
        session.compressionSession,
        kCMTimeInvalid // all pending frames
    );

    return status == noErr;
}

// Destroy encoder and release resources
void vt_encoder_destroy(void* instance) {
    if (!instance) {
        return;
    }

    VTEncoderSession* session = (__bridge_transfer VTEncoderSession*)instance;

    // Flush any pending frames
    if (session.compressionSession) {
        VTCompressionSessionCompleteFrames(session.compressionSession, kCMTimeInvalid);
        VTCompressionSessionInvalidate(session.compressionSession);
        CFRelease(session.compressionSession);
        session.compressionSession = NULL;
    }

    // Callback context will be freed in dealloc
    NSLog(@"VideoToolbox encoder destroyed");
}

} // extern "C"
