#import <AVFoundation/AVFoundation.h>
#import <ImageIO/ImageIO.h>
#include <cstdio>
#include <cstdlib>
#include <cstring>

extern "C" unsigned char *tarantino_video_frame(const char *path, double time_ms,
    unsigned int width, bool lossless, size_t *length, char *error, size_t capacity) {
    @autoreleasepool {
        AVURLAsset *asset = [AVURLAsset URLAssetWithURL:
            [NSURL fileURLWithPath:[NSString stringWithUTF8String:path]] options:nil];
        AVAssetImageGenerator *generator = [AVAssetImageGenerator assetImageGeneratorWithAsset:asset];
        generator.appliesPreferredTrackTransform = YES;
        generator.maximumSize = CGSizeMake(width, 0);
        generator.requestedTimeToleranceBefore = kCMTimeZero;
        generator.requestedTimeToleranceAfter = kCMTimeZero;
        NSError *failure = nil;
        CGImageRef image = [generator copyCGImageAtTime:CMTimeMakeWithSeconds(time_ms / 1000, 60000)
            actualTime:nil error:&failure];
        if (!image) {
            snprintf(error, capacity, "%s", (failure.localizedDescription ?: @"No video frame at requested time").UTF8String);
            return nullptr;
        }
        NSMutableData *data = [NSMutableData data];
        CGImageDestinationRef destination = CGImageDestinationCreateWithData(
            (__bridge CFMutableDataRef)data, lossless ? CFSTR("public.png") : CFSTR("public.jpeg"), 1, nullptr);
        if (!destination) {
            CGImageRelease(image);
            snprintf(error, capacity, "Could not create frame encoder");
            return nullptr;
        }
        CGImageDestinationAddImage(destination, image, (__bridge CFDictionaryRef)@{
            (__bridge NSString *)kCGImageDestinationLossyCompressionQuality: @0.9});
        bool encoded = CGImageDestinationFinalize(destination);
        CFRelease(destination);
        CGImageRelease(image);
        unsigned char *bytes = encoded ? static_cast<unsigned char *>(malloc(data.length)) : nullptr;
        if (!bytes) {
            snprintf(error, capacity, "Could not encode video frame");
            return nullptr;
        }
        memcpy(bytes, data.bytes, data.length);
        *length = data.length;
        return bytes;
    }
}

extern "C" void tarantino_video_frame_free(unsigned char *bytes) { free(bytes); }
