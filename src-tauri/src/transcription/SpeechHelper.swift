import Foundation
import Speech
import AVFoundation
import CoreMedia

struct Word: Codable {
    let id: String
    let text: String
    let startMs: Double
    let endMs: Double
    let type: String
}
struct Transcript: Codable {
    let locale: String
    let words: [Word]
}
struct Capabilities: Codable {
    let available: Bool
    let supportedLocales: [String]
    let installedLocales: [String]
    let selectedLocale: String?
    let modelStatus: String?
}
struct Failure: Codable { let error: String }
enum TranscriptionError: LocalizedError {
    case message(String)
    var errorDescription: String? { if case .message(let text) = self { return text }; return nil }
}
func writeJSON<T: Encodable>(_ value: T) throws {
    let data = try JSONEncoder().encode(value)
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write(Data([10]))
}

struct ModelSetup: Codable { let locale: String; let status: String }
@available(macOS 26.0, *)
func installModel(identifier: String) async throws -> ModelSetup {
    guard SpeechTranscriber.isAvailable,
          let locale = await SpeechTranscriber.supportedLocale(equivalentTo: Locale(identifier: identifier)) else {
        throw TranscriptionError.message("This language or device is not supported by local transcription.")
    }
    // Do not evict another language to make room. Report the system's reservation error.
    try await AssetInventory.reserve(locale: locale)
    let module = SpeechTranscriber(locale: locale, transcriptionOptions: [], reportingOptions: [], attributeOptions: [.audioTimeRange])
    if let installation = try await AssetInventory.assetInstallationRequest(supporting: [module]) {
        try await installation.downloadAndInstall()
    }
    let status = await AssetInventory.status(forModules: [module])
    guard status == .installed else { throw TranscriptionError.message("The speech model is not ready after setup: \(status).") }
    return ModelSetup(locale: locale.identifier, status: "installed")
}

@available(macOS 26.0, *)
func transcribe(path: String, identifier: String) async throws -> Transcript {
    guard SpeechTranscriber.isAvailable else { throw TranscriptionError.message("Local transcription is unavailable on this Mac.") }
    guard let locale = await SpeechTranscriber.supportedLocale(equivalentTo: Locale(identifier: identifier)) else {
        throw TranscriptionError.message("This language is not supported by local transcription.")
    }
    let transcriber = SpeechTranscriber(locale: locale, transcriptionOptions: [], reportingOptions: [], attributeOptions: [.audioTimeRange])
    guard await AssetInventory.status(forModules: [transcriber]) == .installed else {
        throw TranscriptionError.message("The speech model for this language is not installed.")
    }
    let audio = try AVAudioFile(forReading: URL(fileURLWithPath: path))
    let analyzer = SpeechAnalyzer(modules: [transcriber])
    let results = Task { () throws -> [Word] in
        var words: [Word] = []
        for try await result in transcriber.results {
            try Task.checkCancellation()
            for run in result.text.runs {
                let text = String(result.text[run.range].characters).trimmingCharacters(in: .whitespacesAndNewlines)
                if text.isEmpty { continue }
                guard let range = run.audioTimeRange else { throw TranscriptionError.message("The speech engine returned text without timing.") }
                let start = range.start.seconds * 1000
                let end = CMTimeRangeGetEnd(range).seconds * 1000
                guard start.isFinite, end.isFinite, start >= 0, end >= start, text.count <= 10000, words.count < 1_000_000 else {
                    throw TranscriptionError.message("The speech engine returned invalid or oversized transcript data.")
                }
                words.append(Word(id: String(words.count), text: text, startMs: start, endMs: end, type: "word"))
            }
        }
        return words
    }
    do {
        if let end = try await analyzer.analyzeSequence(from: audio) {
            try await analyzer.finalizeAndFinish(through: end)
        } else {
            await analyzer.cancelAndFinishNow()
        }
        return Transcript(locale: locale.identifier, words: try await results.value)
    } catch {
        results.cancel()
        await analyzer.cancelAndFinishNow()
        throw error
    }
}

@main
struct SpeechHelper {
    static func main() async {
        do {
            guard #available(macOS 26.0, *) else { throw TranscriptionError.message("Local transcription requires macOS 26 or later.") }
            let args = Array(CommandLine.arguments.dropFirst())
            if args.first == "status" && args.count <= 2 {
                let available = SpeechTranscriber.isAvailable
                let supported = available ? await SpeechTranscriber.supportedLocales : []
                let installed = available ? await SpeechTranscriber.installedLocales : []
                let locale = args.count == 2 ? await SpeechTranscriber.supportedLocale(equivalentTo: Locale(identifier: args[1])) : nil
                var modelStatus: String? = nil
                if let locale {
                    let module = SpeechTranscriber(locale: locale, transcriptionOptions: [], reportingOptions: [], attributeOptions: [.audioTimeRange])
                    modelStatus = String(describing: await AssetInventory.status(forModules: [module]))
                }
                try writeJSON(Capabilities(available: available, supportedLocales: supported.map(\.identifier).sorted(), installedLocales: installed.map(\.identifier).sorted(), selectedLocale: locale?.identifier, modelStatus: modelStatus))
            } else if args.count == 2 && args[0] == "install" {
                try writeJSON(try await installModel(identifier: args[1]))
            } else if args.count == 3 && args[0] == "transcribe" {
                try writeJSON(try await transcribe(path: args[1], identifier: args[2]))
            } else {
                throw TranscriptionError.message("Expected status [locale], install <locale>, or transcribe <audio file> <locale>.")
            }
        } catch {
            try? writeJSON(Failure(error: error.localizedDescription))
            exit(1)
        }
    }
}
