import Foundation
import Vision
import AppKit

// Usage: yoyo-ocr <image-path>
// Outputs JSON: {"text": "...", "block_count": N}

guard CommandLine.arguments.count > 1 else {
    let err: [String: Any] = ["text": "", "block_count": 0, "error": "Usage: yoyo-ocr <image-path>"]
    if let data = try? JSONSerialization.data(withJSONObject: err),
       let str = String(data: data, encoding: .utf8) {
        print(str)
    }
    exit(1)
}

let imagePath = CommandLine.arguments[1]
let url = URL(fileURLWithPath: imagePath)

guard let image = NSImage(contentsOf: url),
      let tiffData = image.tiffRepresentation,
      let bitmap = NSBitmapImageRep(data: tiffData),
      let cgImage = bitmap.cgImage else {
    let err: [String: Any] = ["text": "", "block_count": 0, "error": "Failed to load image"]
    if let data = try? JSONSerialization.data(withJSONObject: err),
       let str = String(data: data, encoding: .utf8) {
        print(str)
    }
    exit(1)
}

let request = VNRecognizeTextRequest()
request.recognitionLevel = .accurate
request.recognitionLanguages = ["en", "zh-Hans", "zh-Hant"]
request.usesLanguageCorrection = true

let handler = VNImageRequestHandler(cgImage: cgImage, options: [:])
do {
    try handler.perform([request])
} catch {
    let err: [String: Any] = ["text": "", "block_count": 0, "error": "Vision failed: \(error.localizedDescription)"]
    if let data = try? JSONSerialization.data(withJSONObject: err),
       let str = String(data: data, encoding: .utf8) {
        print(str)
    }
    exit(1)
}

// Collect recognized text observations (sorted top-to-bottom by default)
let observations = (request.results as? [VNRecognizedTextObservation]) ?? []

var lines: [String] = []
for obs in observations {
    if let candidate = obs.topCandidates(1).first {
        lines.append(candidate.string)
    }
}

let fullText = lines.joined(separator: "\n")
let output: [String: Any] = [
    "text": fullText,
    "block_count": observations.count
]

if let jsonData = try? JSONSerialization.data(withJSONObject: output, options: []),
   let jsonString = String(data: jsonData, encoding: .utf8) {
    print(jsonString)
} else {
    print("{\"text\":\"\",\"block_count\":0,\"error\":\"JSON serialization failed\"}")
    exit(1)
}
