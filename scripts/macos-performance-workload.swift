// Build with: swiftc -O scripts/macos-performance-workload.swift -o /tmp/TarantinoPerfWorkload
import Cocoa
import OpenGL.GL

final class Workload: NSOpenGLView {
    var timer: Timer?
    var capped = true
    var frames = 0
    var sampleStart = ProcessInfo.processInfo.systemUptime
    let started = ProcessInfo.processInfo.systemUptime
    let log = URL(fileURLWithPath: "/tmp/tarantino-macos-workload.jsonl")

    init?(benchmark: Bool) {
        let attributes: [NSOpenGLPixelFormatAttribute] = [
            UInt32(NSOpenGLPFAAccelerated), UInt32(NSOpenGLPFADoubleBuffer),
            UInt32(NSOpenGLPFAColorSize), 24, 0
        ]
        super.init(frame: NSRect(x: 0, y: 0, width: 960, height: 540),
                   pixelFormat: NSOpenGLPixelFormat(attributes: attributes))
        wantsBestResolutionOpenGLSurface = true
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
    override var acceptsFirstResponder: Bool { true }
    override func prepareOpenGL() {
        super.prepareOpenGL()
        var interval: GLint = 0
        openGLContext?.setValues(&interval, for: .swapInterval)
        setMode(capped: true)
    }
    func setMode(capped: Bool) {
        self.capped = capped
        timer?.invalidate()
        frames = 0
        sampleStart = ProcessInfo.processInfo.systemUptime
        window?.title = "Tarantino Perf Workload — \(capped ? "60 fps" : "uncapped") — Space toggles"
        timer = Timer(timeInterval: capped ? 1.0 / 60.0 : 0.0001, repeats: true) { [weak self] _ in
            self?.render()
        }
        RunLoop.main.add(timer!, forMode: .common)
    }
    override func keyDown(with event: NSEvent) {
        if event.characters == " " { setMode(capped: !capped) }
    }
    func render() {
        openGLContext?.makeCurrentContext()
        let size = convertToBacking(bounds).size
        glViewport(0, 0, GLsizei(size.width), GLsizei(size.height))
        glClearColor(0.04, 0.06, 0.1, 1)
        glClear(GLbitfield(GL_COLOR_BUFFER_BIT))
        glMatrixMode(GLenum(GL_PROJECTION))
        glLoadIdentity()
        glOrtho(-1.777, 1.777, -1, 1, -1, 1)
        glMatrixMode(GLenum(GL_MODELVIEW))
        let now = ProcessInfo.processInfo.systemUptime
        // Moving spokes keep the entire source changing; this is not a game benchmark.
        for gear in 0..<3 {
            glLoadIdentity()
            glTranslatef(Float(gear - 1) * 1.05, Float(gear % 2) * 0.3 - 0.15, 0)
            glRotatef(Float((now - started) * 45) * (gear == 1 ? -1 : 1), 0, 0, 1)
            glBegin(GLenum(GL_TRIANGLES))
            for spoke in 0..<48 {
                let a = Double(spoke) * .pi / 24
                glColor3f(Float(gear == 0 ? 1 : 0.15), Float(gear == 1 ? 1 : 0.2), Float(gear == 2 ? 1 : 0.25))
                glVertex2d(cos(a) * 0.15, sin(a) * 0.15)
                glVertex2d(cos(a) * 0.65, sin(a) * 0.65)
                glVertex2d(cos(a + 0.065) * 0.65, sin(a + 0.065) * 0.65)
            }
            glEnd()
        }
        openGLContext?.flushBuffer()
        frames += 1
        if now - sampleStart >= 5 {
            let row: [String: Any] = ["uptime": now, "capped": capped, "frames": frames,
                "seconds": now - sampleStart, "fps": Double(frames) / (now - sampleStart),
                "width": Int(size.width), "height": Int(size.height)]
            if let data = try? JSONSerialization.data(withJSONObject: row, options: [.sortedKeys]),
               let handle = try? FileHandle(forWritingTo: log) {
                handle.seekToEndOfFile()
                handle.write(data)
                handle.write(Data([10]))
                try? handle.close()
            }
            frames = 0
            sampleStart = now
        }
    }
}
let app = NSApplication.shared
app.setActivationPolicy(.regular)
FileManager.default.createFile(atPath: "/tmp/tarantino-macos-workload.jsonl", contents: nil)
let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 960, height: 540),
    styleMask: [.titled, .closable, .miniaturizable], backing: .buffered, defer: false)
window.title = "Tarantino Perf Workload"
window.contentView = Workload(benchmark: true)
window.center()
window.makeKeyAndOrderFront(nil)
app.activate(ignoringOtherApps: true)
app.run()
