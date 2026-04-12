// clipaste — Fix macOS screenshot paste in terminal AI tools
// https://github.com/hqhq1025/clipaste
//
// macOS screenshots only put image data (TIFF/PNG) on the clipboard,
// but terminals like Ghostty can only Cmd+V paste text or file URLs.
// clipaste watches the clipboard and automatically saves screenshot
// images to temp files, then registers the file URL so Cmd+V works.
//
// This also adds the legacy PNGf pasteboard type that Claude Code
// needs for Ctrl+V image paste via osascript.

import AppKit
import Foundation

let version = "1.1.0"

// MARK: - CLI

if CommandLine.arguments.contains("--version") || CommandLine.arguments.contains("-v") {
    print("clipaste \(version)")
    exit(0)
}

if CommandLine.arguments.contains("--help") || CommandLine.arguments.contains("-h") {
    print("""
    clipaste v\(version) — Fix macOS screenshot paste in terminals

    USAGE
      clipaste            Run in foreground (for brew services / launchd)
      clipaste --version  Print version
      clipaste --help     Show this help

    WHAT IT DOES
      Watches the macOS clipboard. When a screenshot is detected (image data
      without a file URL), clipaste saves it as a temp PNG and registers the
      file path on the clipboard. This lets terminals paste via Cmd+V.

      It also adds the legacy PNGf type so tools like Claude Code can read
      the image via Ctrl+V.

    COMPATIBILITY
      Terminals: Ghostty, Alacritty, iTerm2, Terminal.app, WezTerm, Kitty
      AI tools:  Claude Code, Codex CLI, Cursor CLI

    MORE INFO
      https://github.com/hqhq1025/clipaste
    """)
    exit(0)
}

// MARK: - Pasteboard types

let pngfType = NSPasteboard.PasteboardType("com.apple.pboard.type.PNGf")
let publicPNG = NSPasteboard.PasteboardType("public.png")
let fileURLType = NSPasteboard.PasteboardType("public.file-url")
let filenamesType = NSPasteboard.PasteboardType("NSFilenamesPboardType")

// MARK: - State

let pb = NSPasteboard.general
var lastChangeCount = pb.changeCount
var lastOwnWrite: Int = -1

let tempDir = FileManager.default.temporaryDirectory
    .appendingPathComponent("clipaste", isDirectory: true)

// MARK: - Logging

func log(_ msg: String) {
    let ts = ISO8601DateFormatter().string(from: Date())
    FileHandle.standardError.write(Data("[\(ts)] clipaste: \(msg)\n".utf8))
}

// MARK: - Temp file management

func ensureTempDir() {
    try? FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
}

func savePNGToTemp(_ pngData: Data) -> URL? {
    let timestamp = ISO8601DateFormatter().string(from: Date())
        .replacingOccurrences(of: ":", with: "-")
    let fileURL = tempDir.appendingPathComponent("screenshot-\(timestamp).png")
    do {
        try pngData.write(to: fileURL)
        return fileURL
    } catch {
        log("failed to save temp PNG: \(error)")
        return nil
    }
}

func cleanOldTempFiles() {
    guard let files = try? FileManager.default.contentsOfDirectory(
        at: tempDir, includingPropertiesForKeys: [.creationDateKey]
    ) else { return }

    let cutoff = Date().addingTimeInterval(-3600)
    for file in files {
        guard let attrs = try? file.resourceValues(forKeys: [.creationDateKey]),
              let created = attrs.creationDate,
              created < cutoff else { continue }
        try? FileManager.default.removeItem(at: file)
    }
}

// MARK: - Detection

func isImageOnlyClipboard() -> Bool {
    let types = pb.types ?? []
    let hasImage = types.contains(.tiff) || types.contains(publicPNG)
    let hasFileURL = types.contains(fileURLType)
    let hasFilenames = types.contains(filenamesType)
    let hasString = types.contains(.string)

    return hasImage && !hasFileURL && !hasFilenames && !hasString && types.count <= 6
}

// MARK: - Normalization

func normalizeIfNeeded() {
    let current = pb.changeCount
    guard current != lastChangeCount else { return }
    lastChangeCount = current

    if current == lastOwnWrite { return }

    guard isImageOnlyClipboard() else { return }

    let existingPNG = pb.data(forType: publicPNG)

    let pngData: Data
    if let existing = existingPNG {
        pngData = existing
    } else if let tiffData = pb.data(forType: .tiff),
              let image = NSImage(data: tiffData),
              let tiffRep = image.tiffRepresentation,
              let bitmapRep = NSBitmapImageRep(data: tiffRep),
              let converted = bitmapRep.representation(using: .png, properties: [:]) {
        pngData = converted
    } else {
        log("failed to get PNG data from clipboard")
        return
    }

    guard let fileURL = savePNGToTemp(pngData) else { return }

    // Rewrite clipboard: file URL (for Cmd+V) + PNG/PNGf (for Ctrl+V)
    // Skip TIFF to avoid writing tens of MBs back — saves ~70ms
    pb.clearContents()
    pb.writeObjects([fileURL as NSURL])
    pb.addTypes([publicPNG, pngfType], owner: nil)
    pb.setData(pngData, forType: publicPNG)
    pb.setData(pngData, forType: pngfType)

    lastOwnWrite = pb.changeCount
    lastChangeCount = pb.changeCount
    log("normalized \(fileURL.lastPathComponent) (\(pngData.count) bytes)")

    cleanOldTempFiles()
}

// MARK: - Main

ensureTempDir()
log("v\(version) started (pid \(ProcessInfo.processInfo.processIdentifier))")

let timer = Timer(timeInterval: 0.03, repeats: true) { _ in  // 30ms — fast enough to beat Cmd+V
    normalizeIfNeeded()
}
RunLoop.current.add(timer, forMode: .default)
RunLoop.current.run()
