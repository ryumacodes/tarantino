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
@interface VTEncoderSession : NSObject
@property (nonatomic, assign) VTCompressionSessionRef compressionSession;
@property (nonatomic, assign) VTCallbackContext* callbackContext;
@property (nonatomic, assign) uint32_t width;
@property (nonatomic, assign) uint32_t height;
@property (nonatomic, assign) uint32_t fps;
@property (nonatomic, assign) int64_t frameCount;
@property (nonatomic, assign) CMTimeScale timeScale;
@end

@implementation VTEncoderSession

- (void)dealloc {
    if (self.compressionSession) {
        VTCompressionSessionInvalidate(self.compressionSession);
        CFRelease(self.compressionSession);
        self.compressionSession = NULL;
    }

    if (self.callbackContext) {
        free(self.callbackContext);
        self.callbackContext = NULL;
    }
}

@end

// VideoToolbox compression output callback
static void vtCompressionOutputCallback(
    void* outputCallbackRefCon,
    void* sourceFrameRefCon,
    OSStatus status,
    VTEncodeInfoFlags infoFlags,
    CMSampleBufferRef sampleBuffer
) {
    (void)sourceFrameRefCon; // Unused parameter

    if (status != noErr) {
        NSLog(@"VideoToolbox compression failed with error: %d", (int)status);
        return;
    }

    if (!sampleBuffer) {
        NSLog(@"VideoToolbox produced NULL sample buffer");
        return;
    }

    VTEncoderSession* session = (__bridge VTEncoderSession*)outputCallbackRefCon;
    if (!session || !session.callbackContext || !session.callbackContext->callback) {
        return;
    }

    @autoreleasepool {
        // Check if frame was dropped
        if (infoFlags & kVTEncodeInfo_FrameDropped) {
            NSLog(@"VideoToolbox dropped frame");
            return;
        }

        // Get block buffer containing compressed data
        CMBlockBufferRef blockBuffer = CMSampleBufferGetDataBuffer(sampleBuffer);
        if (!blockBuffer) {
            NSLog(@"Failed to get block buffer from sample");
            return;
        }

        // Get data length
        size_t dataLength = CMBlockBufferGetDataLength(blockBuffer);

        // CRITICAL FIX: Copy the data into a malloc'd buffer that we control
        // The CMBlockBuffer's memory is owned by the CMSampleBuffer and may be released
        // before Rust finishes processing it. We must copy it here.
        uint8_t* copiedData = (uint8_t*)malloc(dataLength);
        if (!copiedData) {
            NSLog(@"Failed to allocate memory for frame data");
            return;
        }

        OSStatus result = CMBlockBufferCopyDataBytes(
            blockBuffer,
            0,                  // offset
            dataLength,         // length
            copiedData          // destination
        );

        if (result != kCMBlockBufferNoErr) {
            NSLog(@"Failed to copy data from block buffer: %d", (int)result);
            free(copiedData);
            return;
        }

        // Get timing information
        CMTime presentationTime = CMSampleBufferGetPresentationTimeStamp(sampleBuffer);
        CMTime decodeTime = CMSampleBufferGetDecodeTimeStamp(sampleBuffer);

        uint64_t timestamp_us = (uint64_t)((presentationTime.value * 1000000) / presentationTime.timescale);
        int64_t pts = presentationTime.value;
        int64_t dts = CMTIME_IS_VALID(decodeTime) ? decodeTime.value : pts;

        // Check if this is a keyframe (sync sample)
        CFArrayRef attachmentsArray = CMSampleBufferGetSampleAttachmentsArray(sampleBuffer, false);
        bool isKeyframe = false;

        if (attachmentsArray && CFArrayGetCount(attachmentsArray) > 0) {
            CFDictionaryRef attachments = (CFDictionaryRef)CFArrayGetValueAtIndex(attachmentsArray, 0);
            CFBooleanRef notSync = (CFBooleanRef)CFDictionaryGetValue(attachments, kCMSampleAttachmentKey_NotSync);
            isKeyframe = !notSync || !CFBooleanGetValue(notSync);
        }

        // Validate NAL units in the encoded frame (AVCC format with 4-byte length prefixes)
        // This helps catch encoder corruption issues early before they reach the muxer
        size_t pos = 0;
        int nalUnitCount = 0;
        int invalidNalCount = 0;

        while (pos + 4 < dataLength) {
            // Read 4-byte length prefix (big-endian)
            uint32_t nalLength = (copiedData[pos] << 24) |
                                (copiedData[pos+1] << 16) |
                                (copiedData[pos+2] << 8) |
                                copiedData[pos+3];
            pos += 4;

            if (nalLength == 0 || pos + nalLength > dataLength) {
                NSLog(@"[VT VALIDATION] Invalid NAL length %u at position %zu (frame size: %zu)",
                      nalLength, pos - 4, dataLength);
                break;
            }

            // Check NAL unit header (first byte after length)
            uint8_t nalHeader = copiedData[pos];
            uint8_t forbiddenBit = (nalHeader >> 7) & 0x1;
            uint8_t nalType = nalHeader & 0x1F;

            if (forbiddenBit != 0) {
                NSLog(@"[VT VALIDATION] WARNING: Forbidden bit set in NAL unit at position %zu", pos);
                invalidNalCount++;
            }

            // Valid NAL types: 1-20 (slice types, parameter sets, etc.)
            // Invalid types: 0 (undefined), 21-23 (reserved), 24-31 (special)
            if (nalType == 0 || (nalType > 20 && nalType < 24)) {
                NSLog(@"[VT VALIDATION] WARNING: Invalid NAL type %u at position %zu (forbidden_bit=%u)",
                      nalType, pos, forbiddenBit);
                invalidNalCount++;
            }

            nalUnitCount++;
            pos += nalLength;
        }

        if (invalidNalCount > 0) {
            NSLog(@"[VT VALIDATION] Frame contains %d/%d corrupted NAL units - encoder may be under stress!",
                  invalidNalCount, nalUnitCount);
        }

        // Extract SPS and PPS from format description
        // This is the CORRECT way to get parameter sets - they're in the format description,
        // not necessarily in every keyframe's bitstream
        CMFormatDescriptionRef formatDesc = CMSampleBufferGetFormatDescription(sampleBuffer);
        uint8_t* sps_copy = NULL;
        size_t sps_size = 0;
        uint8_t* pps_copy = NULL;
        size_t pps_size = 0;

        if (formatDesc) {
            // Get SPS
            const uint8_t* sps_data = NULL;
            size_t sps_count = 0;
            int nal_unit_header_length = 0;

            OSStatus sps_status = CMVideoFormatDescriptionGetH264ParameterSetAtIndex(
                formatDesc,
                0, // SPS index
                &sps_data,
                &sps_size,
                &sps_count,
                &nal_unit_header_length
            );

            if (sps_status == noErr && sps_data && sps_size > 0) {
                // Copy SPS data so it survives beyond this callback
                sps_copy = (uint8_t*)malloc(sps_size);
                if (sps_copy) {
                    memcpy(sps_copy, sps_data, sps_size);
                }
            }

            // Get PPS
            const uint8_t* pps_data = NULL;
            size_t pps_count = 0;

            OSStatus pps_status = CMVideoFormatDescriptionGetH264ParameterSetAtIndex(
                formatDesc,
                1, // PPS index
                &pps_data,
                &pps_size,
                &pps_count,
                &nal_unit_header_length
            );

            if (pps_status == noErr && pps_data && pps_size > 0) {
                // Copy PPS data so it survives beyond this callback
                pps_copy = (uint8_t*)malloc(pps_size);
                if (pps_copy) {
                    memcpy(pps_copy, pps_data, pps_size);
                }
            }
        }

        // Create frame data with our owned copy
        // NOTE: Rust must free this memory after copying it to Bytes
        VTEncodedFrame frame = {
            .data = copiedData,
            .data_len = dataLength,
            .timestamp_us = timestamp_us,
            .is_keyframe = isKeyframe,
            .pts = pts,
            .dts = dts,
            .sps_data = sps_copy,
            .sps_len = sps_copy ? sps_size : 0,
            .pps_data = pps_copy,
            .pps_len = pps_copy ? pps_size : 0
        };

        // Call callback through context
        // The Rust callback MUST copy this data and then free all the buffers
        session.callbackContext->callback(session.callbackContext->rust_context, frame);

        // Free all copied data after the callback returns
        free(copiedData);
        if (sps_copy) free(sps_copy);
        if (pps_copy) free(pps_copy);
    }
}

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
