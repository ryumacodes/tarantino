#import <AVFoundation/AVFoundation.h>
#import <CoreVideo/CoreVideo.h>
#include <cstdio>

struct HoverFrameData {
    const uint8_t *y;
    const uint8_t *uv;
    size_t width;
    size_t height;
    size_t y_stride;
    size_t uv_stride;
    double time_ms;
};

struct HoverReader {
    AVAssetReader *reader;
    AVAssetReaderTrackOutput *output;
    CMSampleBufferRef sample = nullptr;
    CVPixelBufferRef pixels = nullptr;

    void release_frame() {
        if (pixels) CVPixelBufferUnlockBaseAddress(pixels, kCVPixelBufferLock_ReadOnly);
        if (sample) CFRelease(sample);
        pixels = nullptr;
        sample = nullptr;
    }
    ~HoverReader() { release_frame(); }
};

static void reader_error(char *error, size_t capacity, NSString *message) {
    snprintf(error, capacity, "%s", message.UTF8String);
}

extern "C" void *tarantino_hover_reader_open(const char *path, uint32_t width, uint32_t height,
                                             char *error, size_t capacity) {
    @autoreleasepool {
        NSURL *url = [NSURL fileURLWithPath:[NSString stringWithUTF8String:path]];
        AVURLAsset *asset = [AVURLAsset URLAssetWithURL:url options:nil];
        AVAssetTrack *track = [asset tracksWithMediaType:AVMediaTypeVideo].firstObject;
        if (!track) {
            reader_error(error, capacity, @"Recording has no video track");
            return nullptr;
        }
        NSError *failure = nil;
        AVAssetReader *reader = [[AVAssetReader alloc] initWithAsset:asset error:&failure];
        if (!reader) {
            reader_error(error, capacity, failure.localizedDescription ?: @"Cannot open video reader");
            return nullptr;
        }
        NSDictionary *settings = @{
            (id)kCVPixelBufferPixelFormatTypeKey: @(kCVPixelFormatType_420YpCbCr8BiPlanarFullRange),
            (id)kCVPixelBufferWidthKey: @(width),
            (id)kCVPixelBufferHeightKey: @(height)
        };
        AVAssetReaderTrackOutput *output = [[AVAssetReaderTrackOutput alloc] initWithTrack:track outputSettings:settings];
        output.alwaysCopiesSampleData = NO;
        if (![reader canAddOutput:output]) {
            reader_error(error, capacity, @"Cannot decode video for cursor analysis");
            return nullptr;
        }
        [reader addOutput:output];
        if (![reader startReading]) {
            reader_error(error, capacity, reader.error.localizedDescription ?: @"Cannot start video reader");
            return nullptr;
        }
        auto *state = new HoverReader;
        state->reader = reader;
        state->output = output;
        return state;
    }
}

extern "C" int tarantino_hover_reader_next(void *handle, HoverFrameData *frame,
                                            char *error, size_t capacity) {
    @autoreleasepool {
        auto *state = static_cast<HoverReader *>(handle);
        state->release_frame();
        state->sample = [state->output copyNextSampleBuffer];
        if (!state->sample) {
            if (state->reader.status == AVAssetReaderStatusCompleted) return 0;
            reader_error(error, capacity, state->reader.error.localizedDescription ?: @"Video decoding stopped before completion");
            return -1;
        }
        CVPixelBufferRef pixels = CMSampleBufferGetImageBuffer(state->sample);
        if (!pixels || CVPixelBufferGetPixelFormatType(pixels) != kCVPixelFormatType_420YpCbCr8BiPlanarFullRange
            || CVPixelBufferGetPlaneCount(pixels) != 2) {
            reader_error(error, capacity, @"Unsupported decoded video pixel format");
            return -1;
        }
        if (CVPixelBufferLockBaseAddress(pixels, kCVPixelBufferLock_ReadOnly) != kCVReturnSuccess) {
            reader_error(error, capacity, @"Cannot access decoded video planes");
            return -1;
        }
        state->pixels = pixels;
        *frame = {
            static_cast<const uint8_t *>(CVPixelBufferGetBaseAddressOfPlane(pixels, 0)),
            static_cast<const uint8_t *>(CVPixelBufferGetBaseAddressOfPlane(pixels, 1)),
            CVPixelBufferGetWidth(pixels), CVPixelBufferGetHeight(pixels),
            CVPixelBufferGetBytesPerRowOfPlane(pixels, 0), CVPixelBufferGetBytesPerRowOfPlane(pixels, 1),
            CMTimeGetSeconds(CMSampleBufferGetPresentationTimeStamp(state->sample)) * 1000.0
        };
        return 1;
    }
}

extern "C" void tarantino_hover_reader_close(void *handle) {
    @autoreleasepool {
        auto *state = static_cast<HoverReader *>(handle);
        [state->reader cancelReading];
        delete state;
    }
}
