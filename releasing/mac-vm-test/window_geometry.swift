// Read window-server facts for explicit guest process IDs without driving UI.
// Native CUA crops screenshots to the app window, so pixels alone do not show
// whether a drag moved that window. This supplies the missing independent
// position/size observation; it never captures images or changes window state.
// API: https://developer.apple.com/documentation/coregraphics/cgwindowlistcopywindowinfo(_:_:)

import CoreGraphics
import Darwin
import Foundation

/// Refuse unavailable native observations rather than emitting an empty success.
func refuse(_ message: String) -> Never {
    // A missing GUI security session must remain unavailable, not an empty
    // window list that a caller could mistake for a successful observation.
    FileHandle.standardError.write(Data((message + "\n").utf8))
    exit(1)
}

let arguments = Array(CommandLine.arguments.dropFirst())
let processIDs = Set(arguments.compactMap(Int.init))
guard !arguments.isEmpty, processIDs.count == arguments.count, processIDs.allSatisfy({ $0 > 0 }) else {
    refuse("provide distinct positive process IDs for the owned guest app")
}
guard let rows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID)
    as? [[String: Any]] else {
    refuse("native window metadata is unavailable in this GUI security session")
}

// Do not export names, titles, or observations of other guest applications.
let windows: [[String: Any]] = rows.compactMap { row in
    guard let pid = row[kCGWindowOwnerPID as String] as? Int, processIDs.contains(pid),
          let identifier = row[kCGWindowNumber as String] as? Int,
          let layer = row[kCGWindowLayer as String] as? Int,
          let bounds = row[kCGWindowBounds as String] as? [String: Any] else { return nil }
    return ["pid": pid, "window_id": identifier, "layer": layer, "bounds": bounds]
}

var displayCount: UInt32 = 0
guard CGGetActiveDisplayList(0, nil, &displayCount) == .success else {
    refuse("native display inventory is unavailable")
}
let display = CGMainDisplayID()
let bounds = CGDisplayBounds(display)
guard bounds.width > 0, bounds.height > 0 else { refuse("native display bounds are empty") }
let pixelsWide = CGDisplayPixelsWide(display)
let pixelsHigh = CGDisplayPixelsHigh(display)
let observation: [String: Any] = [
    "format": 1,
    "windows": windows,
    "display_count": displayCount,
    "display": [
        "bounds": ["X": bounds.minX, "Y": bounds.minY, "Width": bounds.width, "Height": bounds.height],
        "pixels_wide": pixelsWide,
        "pixels_high": pixelsHigh,
        "pixels_per_unit_x": Double(pixelsWide) / bounds.width,
        "pixels_per_unit_y": Double(pixelsHigh) / bounds.height,
    ],
]
let data = try JSONSerialization.data(withJSONObject: observation, options: [.sortedKeys])
FileHandle.standardOutput.write(data)
FileHandle.standardOutput.write(Data("\n".utf8))
