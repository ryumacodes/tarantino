#pragma once
#include <CoreAudio/CoreAudioTypes.h>
#include <algorithm>
#include <cmath>
#include <cstring>
#include <cstdint>
#include <limits>
#include <vector>

// The capture bridge emits packed, interleaved signed 16-bit PCM to the WAV writer.
inline bool interleave_pcm16(const AudioStreamBasicDescription& format,
                            const AudioBufferList& buffers, size_t frames,
                            std::vector<int16_t>& output) {
    output.clear();
    const size_t channels = format.mChannelsPerFrame;
    const bool floating = format.mFormatFlags & kAudioFormatFlagIsFloat;
    const bool planar = format.mFormatFlags & kAudioFormatFlagIsNonInterleaved;
    const size_t bytes = format.mBitsPerChannel / 8;
    if (format.mFormatID != kAudioFormatLinearPCM || !channels || !frames ||
        (format.mFormatFlags & kAudioFormatFlagIsBigEndian) ||
        !(format.mFormatFlags & kAudioFormatFlagIsPacked) ||
        (floating ? (bytes != 4 && bytes != 8) :
          (!(format.mFormatFlags & kAudioFormatFlagIsSignedInteger) ||
           (bytes != 1 && bytes != 2 && bytes != 4))) ||
        format.mBitsPerChannel != bytes * 8 ||
        frames > std::numeric_limits<size_t>::max() / channels / sizeof(int16_t)) return false;
    const size_t bufferCount = planar ? channels : 1;
    const size_t samplesPerFrame = planar ? 1 : channels;
    if (buffers.mNumberBuffers != bufferCount ||
        format.mBytesPerFrame != bytes * samplesPerFrame) return false;
    for (size_t b = 0; b < bufferCount; ++b) {
        const auto& buffer = buffers.mBuffers[b];
        if (!buffer.mData || buffer.mNumberChannels != samplesPerFrame ||
            frames > buffer.mDataByteSize / format.mBytesPerFrame) return false;
    }
    output.resize(frames * channels);
    for (size_t frame = 0; frame < frames; ++frame) {
        for (size_t channel = 0; channel < channels; ++channel) {
            const auto& buffer = buffers.mBuffers[planar ? channel : 0];
            const auto* source = static_cast<const uint8_t*>(buffer.mData) +
                (frame * samplesPerFrame + (planar ? 0 : channel)) * bytes;
            double sample;
            if (floating && bytes == 4) { float v; std::memcpy(&v, source, 4); sample = v; }
            else if (floating) { double v; std::memcpy(&v, source, 8); sample = v; }
            else if (bytes == 4) { int32_t v; std::memcpy(&v, source, 4); sample = v / 2147483648.0; }
            else if (bytes == 2) { int16_t v; std::memcpy(&v, source, 2); sample = v / 32768.0; }
            else { int8_t v; std::memcpy(&v, source, 1); sample = v / 128.0; }
            output[frame * channels + channel] = !std::isfinite(sample) ? 0 :
                sample >= 1 ? 32767 : sample <= -1 ? -32768 :
                static_cast<int16_t>(std::min(32767.0, std::round(sample * 32768.0)));
        }
    }
    return true;
}
