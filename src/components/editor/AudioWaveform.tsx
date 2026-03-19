import React, { useRef, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface AudioWaveformProps {
  type: 'microphone' | 'system';
  duration: number;
  pixelsPerMs: number;
  audioPath?: string | null;
}

interface WaveformData {
  peaks: number[];
  sampleRate: number;
  channels: number;
}

const AudioWaveform: React.FC<AudioWaveformProps> = ({
  type,
  duration,
  pixelsPerMs,
  audioPath
}) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [waveformData, setWaveformData] = useState<WaveformData | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  // Load waveform data from audio file
  useEffect(() => {
    if (!audioPath) {
      // Generate placeholder waveform data when no audio path
      generatePlaceholderWaveform();
      return;
    }

    setIsLoading(true);

    // Extract real waveform data from audio file
    const loadWaveform = async () => {
      try {
        // Calculate samples per second based on desired resolution
        // For timeline, we want about 100-200 samples per second
        const samplesPerSecond = 100;

        const peaks = await invoke<number[]>('extract_audio_waveform', {
          audioPath,
          samplesPerSecond,
        });

        setWaveformData({
          peaks,
          sampleRate: samplesPerSecond,
          channels: 1,
        });
        console.log(`Loaded ${peaks.length} waveform peaks for ${type} audio`);
      } catch (error) {
        console.error('Failed to load waveform:', error);
        // Fall back to placeholder
        generateRealisticWaveform();
      } finally {
        setIsLoading(false);
      }
    };

    loadWaveform();
  }, [audioPath, duration, type]);

  const generatePlaceholderWaveform = () => {
    const sampleCount = Math.floor(duration / 10); // One sample per 10ms
    const peaks = Array.from({ length: sampleCount }, (_, i) => {
      // Create some variation in the waveform
      const baseLevel = type === 'microphone' ? 0.3 : 0.5;
      const variation = Math.sin(i * 0.1) * 0.2 + Math.random() * 0.3;
      return Math.max(0, Math.min(1, baseLevel + variation));
    });

    setWaveformData({
      peaks,
      sampleRate: 100, // 100 samples per second
      channels: 1
    });
  };

  const generateRealisticWaveform = () => {
    const sampleCount = Math.floor(duration / 5); // Higher resolution
    const peaks = Array.from({ length: sampleCount }, (_, i) => {
      // Simulate more realistic audio patterns
      const time = i / (sampleCount / (duration / 1000)); // Time in seconds
      
      let amplitude = 0;
      
      if (type === 'microphone') {
        // Simulate speech patterns with pauses
        const speechPattern = Math.sin(time * 0.3) > 0.2 ? 1 : 0.1;
        amplitude = speechPattern * (0.4 + Math.random() * 0.4) * Math.sin(time * 20);
      } else {
        // Simulate system audio (more consistent)
        amplitude = (0.6 + Math.sin(time * 0.5) * 0.2) * Math.sin(time * 15);
      }
      
      return Math.max(0, Math.min(1, Math.abs(amplitude)));
    });

    setWaveformData({
      peaks,
      sampleRate: 200, // 200 samples per second
      channels: 1
    });
  };

  // Draw waveform on canvas
  useEffect(() => {
    if (!canvasRef.current || !waveformData) return;

    const canvas = canvasRef.current;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const { width, height } = canvas;
    const { peaks } = waveformData;

    // Clear canvas
    ctx.clearRect(0, 0, width, height);

    if (peaks.length === 0) return;

    // Calculate dimensions
    const samplesPerPixel = peaks.length / width;
    const centerY = height / 2;
    const maxAmplitude = height / 2 - 2; // Leave some padding

    // Set waveform color based on type
    const gradient = ctx.createLinearGradient(0, 0, 0, height);
    if (type === 'microphone') {
      gradient.addColorStop(0, 'rgba(80, 250, 123, 0.8)'); // Dracula green
      gradient.addColorStop(1, 'rgba(80, 250, 123, 0.2)');
    } else {
      gradient.addColorStop(0, 'rgba(139, 233, 253, 0.8)'); // Dracula cyan
      gradient.addColorStop(1, 'rgba(139, 233, 253, 0.2)');
    }
    
    ctx.fillStyle = gradient;
    ctx.strokeStyle = type === 'microphone' ? '#50fa7b' : '#8be9fd';
    ctx.lineWidth = 1;

    // Draw waveform
    ctx.beginPath();
    
    for (let x = 0; x < width; x++) {
      const sampleIndex = Math.floor(x * samplesPerPixel);
      const sample = peaks[sampleIndex] || 0;
      const y = centerY - (sample * maxAmplitude);
      
      if (x === 0) {
        ctx.moveTo(x, y);
      } else {
        ctx.lineTo(x, y);
      }
    }
    
    // Mirror the waveform for symmetry
    for (let x = width - 1; x >= 0; x--) {
      const sampleIndex = Math.floor(x * samplesPerPixel);
      const sample = peaks[sampleIndex] || 0;
      const y = centerY + (sample * maxAmplitude);
      ctx.lineTo(x, y);
    }
    
    ctx.closePath();
    ctx.fill();
    
    // Draw center line
    ctx.beginPath();
    ctx.moveTo(0, centerY);
    ctx.lineTo(width, centerY);
    ctx.strokeStyle = type === 'microphone' ? 'rgba(80, 250, 123, 0.3)' : 'rgba(139, 233, 253, 0.3)';
    ctx.lineWidth = 1;
    ctx.stroke();

  }, [waveformData, type]);

  const waveformWidth = Math.max(duration * pixelsPerMs, 100);

  return (
    <div 
      className="audio-waveform-container"
      style={{
        width: waveformWidth,
        height: '100%',
        position: 'relative',
        minWidth: '100px'
      }}
    >
      <canvas
        ref={canvasRef}
        width={waveformWidth}
        height={40}
        style={{
          width: '100%',
          height: '100%',
          opacity: isLoading ? 0.5 : 1,
          transition: 'opacity 0.3s ease'
        }}
      />
      {isLoading && (
        <div style={{
          position: 'absolute',
          top: '50%',
          left: '50%',
          transform: 'translate(-50%, -50%)',
          fontSize: '10px',
          color: 'var(--editor-text-secondary)'
        }}>
          Loading...
        </div>
      )}
    </div>
  );
};

export default AudioWaveform;