import Foundation
import Speech

// Usage: yoyo-speech <wav_file_path> [locale]
// Transcribes a WAV audio file using SFSpeechRecognizer.
// Outputs transcribed text to stdout, errors to stderr.

guard CommandLine.arguments.count >= 2 else {
    fputs("Usage: yoyo-speech <wav_file_path> [locale]\n", stderr)
    exit(1)
}

let filePath = CommandLine.arguments[1]
let localeId = CommandLine.arguments.count >= 3 ? CommandLine.arguments[2] : "en-US"

let fileURL = URL(fileURLWithPath: filePath)

guard FileManager.default.fileExists(atPath: filePath) else {
    fputs("ERROR:File not found: \(filePath)\n", stderr)
    exit(1)
}

// Check authorization
let authStatus = SFSpeechRecognizer.authorizationStatus()
guard authStatus == .authorized else {
    fputs("ERROR:Speech recognition not authorized (status: \(authStatus.rawValue))\n", stderr)
    exit(2)
}

guard let recognizer = SFSpeechRecognizer(locale: Locale(identifier: localeId)) else {
    fputs("ERROR:Could not create recognizer for locale: \(localeId)\n", stderr)
    exit(3)
}

guard recognizer.isAvailable else {
    fputs("ERROR:Speech recognition not available for locale: \(localeId)\n", stderr)
    exit(4)
}

let request = SFSpeechURLRecognitionRequest(url: fileURL)
let semaphore = DispatchSemaphore(value: 0)
var transcribedText = ""
var recognitionError: Error?

recognizer.recognitionTask(with: request) { result, error in
    if let error = error {
        recognitionError = error
        semaphore.signal()
        return
    }

    guard let result = result else { return }

    if result.isFinal {
        transcribedText = result.bestTranscription.formattedString
        semaphore.signal()
    }
}

// Wait up to 30 seconds
let timeout = semaphore.wait(timeout: .now() + 30)

if timeout == .timedOut {
    fputs("ERROR:Transcription timed out\n", stderr)
    exit(5)
}

if let error = recognitionError {
    fputs("ERROR:\(error.localizedDescription)\n", stderr)
    exit(6)
}

print(transcribedText)
