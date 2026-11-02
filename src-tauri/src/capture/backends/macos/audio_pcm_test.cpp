#include "audio_pcm.h"
#include <cassert>
#include <iostream>

int main() {
    AudioStreamBasicDescription format{};
    format.mFormatID = kAudioFormatLinearPCM;
    format.mFormatFlags = kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked | kAudioFormatFlagIsNonInterleaved;
    format.mSampleRate = 48000;
    format.mChannelsPerFrame = 2;
    format.mBitsPerChannel = 32;
    format.mBytesPerFrame = 4;
    struct { UInt32 count; AudioBuffer buffers[2]; } planar;
    float left[] = {0.5f, -0.5f, 2.f, std::numeric_limits<float>::quiet_NaN()};
    float right[] = {-1.f, 1.f, -2.f, std::numeric_limits<float>::infinity()};
    planar.count = 2;
    planar.buffers[0] = {1, sizeof(left), left};
    planar.buffers[1] = {1, sizeof(right), right};
    const auto& list = *reinterpret_cast<AudioBufferList*>(&planar);
    std::vector<int16_t> pcm;
    assert(interleave_pcm16(format, list, 4, pcm));
    assert((pcm == std::vector<int16_t>{16384,-32768,-16384,32767,32767,-32768,0,0}));
    // Exactly one stereo output frame per source frame, regardless of float byte width.
    assert(pcm.size() * sizeof(int16_t) == 4 * 2 * 2);
    planar.buffers[1].mDataByteSize = 4;
    assert(!interleave_pcm16(format, list, 4, pcm) && pcm.empty());
    planar.buffers[1].mDataByteSize = sizeof(right);
    format.mFormatFlags &= ~kAudioFormatFlagIsNonInterleaved;
    format.mBytesPerFrame = 8;
    float packed[] = {0.5f, -1.f, -0.5f, 1.f};
    AudioBufferList interleaved{1, {{2, sizeof(packed), packed}}};
    assert(interleave_pcm16(format, interleaved, 2, pcm));
    assert((pcm == std::vector<int16_t>{16384,-32768,-16384,32767}));
    int16_t integers[] = {-32768,-1,0,1,32766,32767};
    format.mFormatFlags = kAudioFormatFlagIsSignedInteger | kAudioFormatFlagIsPacked;
    format.mBitsPerChannel = 16;
    format.mBytesPerFrame = 4;
    interleaved.mBuffers[0] = {2, sizeof(integers), integers};
    assert(interleave_pcm16(format, interleaved, 3, pcm));
    assert(pcm == std::vector<int16_t>(integers, integers + 6));
    format.mFormatFlags |= kAudioFormatFlagIsBigEndian;
    assert(!interleave_pcm16(format, interleaved, 3, pcm));
    std::cout << "Audio PCM conversion tests passed\n";
}
