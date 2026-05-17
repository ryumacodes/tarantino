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
