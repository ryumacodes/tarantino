/**
 * TARANTINO WEBGL PREVIEW BRIDGE
 * JavaScript bridge for Three.js/WebGL preview in Tauri webview
 * Auto-enables on Windows/Chromium, falls back to native on macOS/WebKit
 */

import * as THREE from 'three';

class TarantinoWebGLPreview {
    constructor(canvasId = 'tarantino-preview-canvas') {
        this.canvasId = canvasId;
        this.canvas = null;
        this.renderer = null;
        this.scene = null;
        this.camera = null;
        this.videoTextures = new Map();
        this.videoPlanes = new Map();
        this.cursorObject = null;
        this.isInitialized = false;
        this.isPlaying = false;
        this.currentTime = 0;
        this.playbackSpeed = 1.0;
        this.stats = {
            framesRendered: 0,
            fps: 0,
            lastFrameTime: 0,
            droppedFrames: 0
        };
        
        // WebGL capabilities
        this.capabilities = {
            webglVersion: null,
            extensions: [],
            maxTextureSize: 0,
            hardwareAcceleration: false
        };
        
        // Frame processing
        this.frameQueue = [];
        this.lastFrameId = null;
        
        console.log('TarantinoWebGLPreview initialized');
    }
    
    /**
     * Initialize WebGL preview engine
     */
    async initialize() {
        try {
            // Get canvas element
            this.canvas = document.getElementById(this.canvasId);
            if (!this.canvas) {
                throw new Error(`Canvas element '${this.canvasId}' not found`);
            }
            
            // Detect WebGL capabilities
            await this.detectCapabilities();
            
            // Initialize Three.js renderer
            this.initializeRenderer();
            
            // Initialize scene
            this.initializeScene();
            
            // Start render loop
            this.startRenderLoop();
            
            // Report capabilities to Rust backend
            await this.reportCapabilities();
            
            this.isInitialized = true;
            console.log('WebGL preview initialized successfully');
            
            return true;
        } catch (error) {
            console.error('Failed to initialize WebGL preview:', error);
            return false;
        }
    }
    
    /**
     * Detect WebGL capabilities
     */
    async detectCapabilities() {
        const gl = this.canvas.getContext('webgl2') || this.canvas.getContext('webgl');
        
        if (!gl) {
            throw new Error('WebGL not supported');
        }
        
        this.capabilities.webglVersion = gl instanceof WebGL2RenderingContext ? 'WebGL2' : 'WebGL1';
        this.capabilities.extensions = gl.getSupportedExtensions() || [];
        this.capabilities.maxTextureSize = gl.getParameter(gl.MAX_TEXTURE_SIZE);
        
        // Check for hardware acceleration
        const debugInfo = gl.getExtension('WEBGL_debug_renderer_info');
        if (debugInfo) {
            const renderer = gl.getParameter(debugInfo.UNMASKED_RENDERER_WEBGL);
            this.capabilities.hardwareAcceleration = !renderer.includes('Software');
        }
        
        console.log('WebGL capabilities detected:', this.capabilities);
    }
    
    /**
     * Initialize Three.js renderer
     */
    initializeRenderer() {
        this.renderer = new THREE.WebGLRenderer({
            canvas: this.canvas,
            antialias: true,
            alpha: false,
            powerPreference: 'high-performance'
        });
        
        this.renderer.setSize(this.canvas.clientWidth, this.canvas.clientHeight);
        this.renderer.setPixelRatio(window.devicePixelRatio);
        this.renderer.setClearColor(0x000000, 1.0); // Black background
        
        // Enable useful features
        this.renderer.shadowMap.enabled = false; // Disable shadows for performance
        this.renderer.outputColorSpace = THREE.SRGBColorSpace;
    }
    
    /**
     * Initialize Three.js scene
     */
    initializeScene() {
        // Create scene
        this.scene = new THREE.Scene();
        
        // Create camera
        this.camera = new THREE.PerspectiveCamera(
            75, // Field of view
            this.canvas.clientWidth / this.canvas.clientHeight, // Aspect ratio
            0.1, // Near clipping plane
            1000 // Far clipping plane
        );
        this.camera.position.z = 1;
        
        // Add ambient light
        const ambientLight = new THREE.AmbientLight(0x404040, 0.4);
        this.scene.add(ambientLight);
        
        // Add directional light
        const directionalLight = new THREE.DirectionalLight(0xffffff, 0.8);
        directionalLight.position.set(1, 1, 1);
        this.scene.add(directionalLight);
        
        console.log('Three.js scene initialized');
    }
    
    /**
     * Load project for preview
     */
    async loadProject(projectData) {
        console.log('Loading project in WebGL preview:', projectData.id);
        
        // Clear existing content
        this.clearScene();
        
        // Create video planes for each clip
        for (let i = 0; i < projectData.clips.length; i++) {
            const clip = projectData.clips[i];
            if (clip.tracks?.video) {
                await this.createVideoPlane(i, clip.tracks.video);
            }
        }
        
        // Create cursor object if cursor events exist
        if (projectData.cursor_events && projectData.cursor_events.length > 0) {
            this.createCursorObject();
        }
        
        console.log(`Loaded ${projectData.clips.length} clips in WebGL scene`);
    }
    
    /**
     * Create video plane for a clip
     */
    async createVideoPlane(index, videoTrack) {
        const planeId = `video_plane_${index}`;
        const textureId = `video_texture_${index}`;
        
        // Create video texture
        const videoTexture = new THREE.VideoTexture(null);
        videoTexture.minFilter = THREE.LinearFilter;
        videoTexture.magFilter = THREE.LinearFilter;
        videoTexture.format = THREE.RGBFormat;
        
        this.videoTextures.set(textureId, videoTexture);
        
        // Create plane geometry
        const geometry = new THREE.PlaneGeometry(
            videoTrack.width / 1000, // Scale down for Three.js coordinates
            videoTrack.height / 1000
        );
        
        // Create material with video texture
        const material = new THREE.MeshBasicMaterial({
            map: videoTexture,
            transparent: false
        });
        
        // Create mesh
        const plane = new THREE.Mesh(geometry, material);
        plane.position.set(0, 0, 0);
        
        this.scene.add(plane);
        this.videoPlanes.set(planeId, plane);
        
        console.log(`Created video plane: ${planeId} (${videoTrack.width}x${videoTrack.height})`);
    }
    
    /**
     * Create cursor visualization
     */
    createCursorObject() {
        const geometry = new THREE.CircleGeometry(0.02, 16); // Small circle
        const material = new THREE.MeshBasicMaterial({
            color: 0xffffff,
            transparent: true,
            opacity: 0.8
        });
        
        this.cursorObject = new THREE.Mesh(geometry, material);
        this.cursorObject.position.z = 0.1; // Slightly in front of video
        
        this.scene.add(this.cursorObject);
        console.log('Created cursor object');
    }
    
    /**
     * Update video frame
     */
    updateVideoFrame(videoId, frameData, timestamp) {
        const textureId = `video_texture_${videoId}`;
        const texture = this.videoTextures.get(textureId);
        
        if (texture && frameData) {
            // Convert frame data to ImageData or Video element
            // This would be implemented with proper frame decoding
            console.log(`Updated video frame: ${textureId} at ${timestamp}ms`);
            
            // Mark texture as needing update
            texture.needsUpdate = true;
        }
    }
    
    /**
     * Update cursor position and animation
     */
    updateCursor(x, y, timestamp, animation = 'idle') {
        if (!this.cursorObject) return;
        
        // Convert screen coordinates to WebGL coordinates
        const normalizedX = (x / this.canvas.clientWidth) * 2 - 1;
        const normalizedY = -((y / this.canvas.clientHeight) * 2 - 1);
        
        this.cursorObject.position.x = normalizedX;
        this.cursorObject.position.y = normalizedY;
        
        // Update cursor appearance based on animation
        switch (animation) {
            case 'click':
                this.cursorObject.material.opacity = 1.0;
                this.cursorObject.scale.setScalar(1.2);
                break;
            case 'hover':
                this.cursorObject.material.opacity = 0.9;
                this.cursorObject.scale.setScalar(1.1);
                break;
            case 'move':
                this.cursorObject.material.opacity = 0.7;
                this.cursorObject.scale.setScalar(1.0);
                break;
            default: // idle
                this.cursorObject.material.opacity = 0.8;
                this.cursorObject.scale.setScalar(1.0);
        }
    }
    
    /**
     * Apply zoom effect
     */
    applyZoomEffect(zoomFactor, focusX, focusY, progress, easing = 'easeInOut') {
        if (!this.camera) return;
        
        // Apply easing function
        const easedProgress = this.applyEasing(progress, easing);
        
        // Calculate zoom via field of view adjustment
        const baseFov = 75;
        const targetFov = baseFov / zoomFactor;
        this.camera.fov = baseFov + (targetFov - baseFov) * easedProgress;
        
        // Apply focus point adjustment
        const focusWorldX = (focusX * 2 - 1) * (1 - easedProgress);
        const focusWorldY = -((focusY * 2 - 1) * (1 - easedProgress));
        
        this.camera.position.x = focusWorldX * 0.1;
        this.camera.position.y = focusWorldY * 0.1;
        
        this.camera.updateProjectionMatrix();
        
        console.log(`Applied zoom: ${zoomFactor.toFixed(2)}x at (${focusX.toFixed(2)}, ${focusY.toFixed(2)}), progress: ${progress.toFixed(2)}`);
    }
    
    /**
     * Apply easing function
     */
    applyEasing(progress, easing) {
        switch (easing) {
            case 'linear':
                return progress;
            case 'easeIn':
                return progress * progress;
            case 'easeOut':
                return 1 - (1 - progress) * (1 - progress);
            case 'easeInOut':
                return progress < 0.5 ? 
                    2 * progress * progress : 
                    1 - Math.pow(-2 * progress + 2, 2) / 2;
            case 'bounce':
                return progress + Math.sin(progress * Math.PI * 4) * 0.1;
            default:
                return progress;
        }
    }
    
    /**
     * Start render loop
     */
    startRenderLoop() {
        const render = (timestamp) => {
            if (!this.isInitialized) return;
            
            // Update stats
            this.updateStats(timestamp);
            
            // Render scene
            this.renderer.render(this.scene, this.camera);
            
            // Continue loop
            this.lastFrameId = requestAnimationFrame(render);
        };
        
        render(performance.now());
        console.log('Render loop started');
    }
    
    /**
     * Update rendering statistics
     */
    updateStats(timestamp) {
        this.stats.framesRendered++;
        
        if (this.stats.lastFrameTime > 0) {
            const deltaTime = timestamp - this.stats.lastFrameTime;
            this.stats.fps = 1000 / deltaTime;
        }
        
        this.stats.lastFrameTime = timestamp;
        
        // Report performance periodically
        if (this.stats.framesRendered % 60 === 0) {
            this.reportPerformance();
        }
    }
    
    /**
     * Clear scene content
     */
    clearScene() {
        // Remove video planes
        for (const [id, plane] of this.videoPlanes) {
            this.scene.remove(plane);
            plane.geometry.dispose();
            plane.material.dispose();
        }
        this.videoPlanes.clear();
        
        // Dispose video textures
        for (const [id, texture] of this.videoTextures) {
            texture.dispose();
        }
        this.videoTextures.clear();
        
        // Remove cursor object
        if (this.cursorObject) {
            this.scene.remove(this.cursorObject);
            this.cursorObject.geometry.dispose();
            this.cursorObject.material.dispose();
            this.cursorObject = null;
        }
        
        console.log('Scene cleared');
    }
    
    /**
     * Handle window resize
     */
    handleResize() {
        if (!this.renderer || !this.camera) return;
        
        const width = this.canvas.clientWidth;
        const height = this.canvas.clientHeight;
        
        this.renderer.setSize(width, height);
        this.camera.aspect = width / height;
        this.camera.updateProjectionMatrix();
    }
    
    /**
     * Report capabilities to Rust backend
     */
    async reportCapabilities() {
        try {
            const capabilities = {
                webgl_version: this.capabilities.webglVersion,
                extensions: this.capabilities.extensions,
                max_texture_size: this.capabilities.maxTextureSize,
                hardware_acceleration: this.capabilities.hardwareAcceleration
            };
            
            // This would use Tauri's invoke to report to Rust
            console.log('Reporting capabilities to backend:', capabilities);
        } catch (error) {
            console.error('Failed to report capabilities:', error);
        }
    }
    
    /**
     * Report performance metrics
     */
    async reportPerformance() {
        try {
            const performance = {
                fps: this.stats.fps,
                frame_time_ms: 1000 / this.stats.fps,
                dropped_frames: this.stats.droppedFrames
            };
            
            console.log('Performance:', performance);
        } catch (error) {
            console.error('Failed to report performance:', error);
        }
    }
    
    /**
     * Cleanup resources
     */
    destroy() {
        // Cancel render loop
        if (this.lastFrameId) {
            cancelAnimationFrame(this.lastFrameId);
        }
        
        // Clear scene
        this.clearScene();
        
        // Dispose renderer
        if (this.renderer) {
            this.renderer.dispose();
        }
        
        this.isInitialized = false;
        console.log('WebGL preview destroyed');
    }
}

// Global instance
window.tarantinoWebGL = new TarantinoWebGLPreview();

// Auto-initialize when DOM is ready
document.addEventListener('DOMContentLoaded', async () => {
    const success = await window.tarantinoWebGL.initialize();
    if (success) {
        console.log('✅ WebGL preview ready');
    } else {
        console.log('❌ WebGL preview failed, falling back to native');
    }
});

// Handle window resize
window.addEventListener('resize', () => {
    window.tarantinoWebGL.handleResize();
});

export default TarantinoWebGLPreview;