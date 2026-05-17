@interface SCKCaptureSession : NSObject <SCStreamOutput, SCStreamDelegate>
@property (nonatomic, strong) SCStream* stream;
@property (nonatomic, strong) SCContentFilter* filter;
@property (nonatomic, strong) SCStreamConfiguration* config;
@property (nonatomic, assign) SCKCallbackContext* callbackContext;
@property (nonatomic, assign) BOOL isCapturing;
@property (nonatomic, assign) BOOL startupFailed;
@property (nonatomic, assign) dispatch_semaphore_t startupSemaphore;

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
    if (!self.isCapturing) {
        self.startupFailed = YES;
        if (self.startupSemaphore) {
            dispatch_semaphore_signal(self.startupSemaphore);
            self.startupSemaphore = nil;
        }
    }
    self.isCapturing = NO;
}

@end

// C API implementation
