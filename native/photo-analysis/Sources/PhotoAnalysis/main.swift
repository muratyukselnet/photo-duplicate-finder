import Foundation
import CoreGraphics
import ImageIO
import UniformTypeIdentifiers
import Vision

// JSON-lines protocol v1. All coordinates use Vision's normalized bottom-left origin.
let extractorVersion = "imageio-vision-v2-" + ProcessInfo.processInfo.operatingSystemVersionString
struct HelperError: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}

func source(_ path: String) throws -> CGImageSource {
    guard let image = CGImageSourceCreateWithURL(URL(fileURLWithPath: path) as CFURL, nil) else {
        throw HelperError(message: "ImageIO could not open image")
    }
    return image
}

func metadata(_ image: CGImageSource) -> [String: Any] {
    guard let properties = CGImageSourceCopyPropertiesAtIndex(image, 0, nil) as? [CFString: Any] else { return [:] }
    let exif = properties[kCGImagePropertyExifDictionary] as? [CFString: Any] ?? [:]
    var result: [String: Any] = [:]
    if let orientation = properties[kCGImagePropertyOrientation] as? Int { result["orientation"] = orientation }
    for (key, name) in [(kCGImagePropertyExifFocalLength, "focal_length"),
                        (kCGImagePropertyExifFNumber, "aperture"),
                        (kCGImagePropertyExifExposureBiasValue, "exposure_bias"),
                        (kCGImagePropertyExifLensModel, "lens"),
                        (kCGImagePropertyExifSubjectArea, "subject_area")] {
        if let value = exif[key] { result[name] = value }
    }
    return result
}

func dimensions(_ image: CGImageSource) throws -> (Int, Int) {
    guard let properties = CGImageSourceCopyPropertiesAtIndex(image, 0, nil) as? [CFString: Any],
          let width = properties[kCGImagePropertyPixelWidth] as? Int,
          let height = properties[kCGImagePropertyPixelHeight] as? Int else {
        throw HelperError(message: "ImageIO could not read image dimensions")
    }
    let orientation = properties[kCGImagePropertyOrientation] as? Int ?? 1
    return (5...8).contains(orientation) ? (height, width) : (width, height)
}

// Always generate from the image rather than use an embedded thumbnail. ImageIO applies
// EXIF orientation once, and the destination is written without source metadata.
func decode(_ image: CGImageSource, edge: Int) throws -> CGImage {
    let options: [CFString: Any] = [
        kCGImageSourceCreateThumbnailFromImageAlways: true,
        kCGImageSourceCreateThumbnailWithTransform: true,
        kCGImageSourceThumbnailMaxPixelSize: edge,
        kCGImageSourceShouldCacheImmediately: true
    ]
    guard let decoded = CGImageSourceCreateThumbnailAtIndex(image, 0, options as CFDictionary) else {
        throw HelperError(message: "ImageIO could not decode image")
    }
    return decoded
}

func rect(_ box: CGRect, confidence: Float) -> [String: Any] {
    ["x": box.origin.x, "y": box.origin.y, "width": box.width,
     "height": box.height, "confidence": confidence]
}

// Laplacian variance measures image high-frequency content, not focus or blur by itself.
// Clipping counts luminance values at the ends of an 8-bit grayscale rendering.
func metrics(_ image: CGImage) throws -> [String: Any] {
    let width = image.width, height = image.height
    var pixels = [UInt8](repeating: 0, count: width * height)
    guard let context = CGContext(data: &pixels, width: width, height: height,
        bitsPerComponent: 8, bytesPerRow: width, space: CGColorSpaceCreateDeviceGray(),
        bitmapInfo: CGImageAlphaInfo.none.rawValue) else {
        throw HelperError(message: "Could not create analysis bitmap")
    }
    context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
    let total = Double(pixels.count)
    var result: [String: Any] = [
        "highlight_clipping": Double(pixels.filter { $0 >= 250 }.count) / total,
        "shadow_clipping": Double(pixels.filter { $0 <= 5 }.count) / total
    ]
    if width > 2 && height > 2 {
        var sum = 0.0, sumSquares = 0.0, count = 0.0
        for y in 1..<(height - 1) {
            for x in 1..<(width - 1) {
                let i = y * width + x
                let lap = Double(Int(pixels[i - 1]) + Int(pixels[i + 1]) + Int(pixels[i - width]) + Int(pixels[i + width]) - 4 * Int(pixels[i]))
                sum += lap; sumSquares += lap * lap; count += 1
            }
        }
        result["sharpness"] = max(0, sumSquares / count - (sum / count) * (sum / count))
    }
    return result
}

func analyze(_ path: String) throws -> [String: Any] {
    let imageSource = try source(path)
    let (width, height) = try dimensions(imageSource)
    let image = try decode(imageSource, edge: 1600)
    var result = try metrics(image)
    result["extractor_version"] = extractorVersion
    result["width"] = width; result["height"] = height
    result["analysis_width"] = image.width; result["analysis_height"] = image.height
    let metadata = metadata(imageSource)
    result["metadata"] = metadata
    var subject: (CGRect, String)?
    let handler = VNImageRequestHandler(cgImage: image, options: [:])
    let faces = VNDetectFaceCaptureQualityRequest()
    if (try? handler.perform([faces])) != nil {
        if let face = faces.results?.max(by: { $0.boundingBox.width * $0.boundingBox.height < $1.boundingBox.width * $1.boundingBox.height }) {
            subject = (face.boundingBox, "face")
        }
        result["faces"] = (faces.results ?? []).map { face -> [String: Any] in
            var value = rect(face.boundingBox, confidence: face.confidence)
            if let quality = face.faceCaptureQuality { value["quality"] = quality }
            return value
        }
    }
    let saliency = VNGenerateAttentionBasedSaliencyImageRequest()
    if (try? handler.perform([saliency])) != nil {
        if subject == nil, let object = saliency.results?.first?.salientObjects?.first {
            subject = (object.boundingBox, "saliency")
        }
        result["subjects"] = (saliency.results?.first?.salientObjects ?? []).map {
            rect($0.boundingBox, confidence: $0.confidence)
        }
    }
    if #available(macOS 15.0, *) {
        let aesthetics = VNCalculateImageAestheticsScoresRequest()
        if (try? handler.perform([aesthetics])) != nil, let observation = aesthetics.results?.first {
            result["aesthetics_score"] = observation.overallScore
            result["is_utility"] = observation.isUtility
        }
    }
    // EXIF SubjectArea is expressed in unrotated source pixels. Only use it as a
    // crop location for orientation 1; expose it as metadata for other orientations.
    if subject == nil, (metadata["orientation"] as? Int ?? 1) == 1,
       let area = metadata["subject_area"] as? [NSNumber], area.count >= 2 {
        let x = area[0].doubleValue / Double(width)
        let y = 1 - area[1].doubleValue / Double(height)
        if (0...1).contains(x) && (0...1).contains(y) {
            subject = (CGRect(x: x, y: y, width: 0, height: 0), "focus_area")
        }
    }
    // Decode at source resolution before extracting crops. Large images are
    // skipped instead of silently measuring a resampled crop or exhausting RAM.
    if let (box, origin) = subject, width <= 8192, height <= 8192,
       width * height <= 60_000_000, let full = try? decode(imageSource, edge: max(width, height)) {
        let edge = min(384, full.width, full.height)
        if edge >= 16 {
            func cropRect(at point: CGPoint) -> CGRect {
                let x = max(0, min(full.width - edge, Int(point.x * CGFloat(full.width)) - edge / 2))
                let y = max(0, min(full.height - edge, Int((1 - point.y) * CGFloat(full.height)) - edge / 2))
                return CGRect(x: x, y: y, width: edge, height: edge)
            }
            let center = CGPoint(x: box.midX, y: box.midY)
            let subjectRect = cropRect(at: center)
            if let region = full.cropping(to: subjectRect), let values = try? metrics(region), let sharpness = values["sharpness"] {
                result["subject_sharpness"] = sharpness
                result["subject_source"] = origin
                result["sharpness_crop_size"] = edge
            }
            // Select the far corner, but omit it when a saliency/face box covers it.
            let background = CGPoint(x: center.x < 0.5 ? 0.9 : 0.1, y: center.y < 0.5 ? 0.9 : 0.1)
            let backgroundRect = cropRect(at: background)
            let objectRect = CGRect(x: box.minX * CGFloat(full.width), y: (1 - box.maxY) * CGFloat(full.height),
                                    width: box.width * CGFloat(full.width), height: box.height * CGFloat(full.height))
            if !subjectRect.intersects(backgroundRect), !objectRect.intersects(backgroundRect),
               let region = full.cropping(to: backgroundRect),
               let values = try? metrics(region), let sharpness = values["sharpness"] {
                result["background_sharpness"] = sharpness
            }
        }
    }
    let feature = VNGenerateImageFeaturePrintRequest()
    if (try? handler.perform([feature])) != nil, let print = feature.results?.first {
        result["feature_print"] = print.data.base64EncodedString()
        result["feature_print_element_count"] = print.elementCount
        result["feature_print_element_type"] = print.elementType.rawValue
        result["feature_print_revision"] = print.requestRevision
    }
    return result
}

func run(_ request: [String: Any]) throws -> [String: Any] {
    guard let command = request["command"] as? String else { throw HelperError(message: "Missing command") }
    if command == "capabilities" {
        var result: [String: Any] = ["extractor_version": extractorVersion, "imageio": true, "vision": true,
                "face_quality": true, "saliency": true, "feature_print": true, "aesthetics": false]
        if #available(macOS 15.0, *) { result["aesthetics"] = true }
        return result
    }
    guard let path = request["path"] as? String, !path.isEmpty else { throw HelperError(message: "Missing path") }
    switch command {
    case "analyze": return try analyze(path)
    case "dimensions":
        let (width, height) = try dimensions(source(path))
        return ["width": width, "height": height]
    case "render":
        guard let output = request["output_path"] as? String, !output.isEmpty else { throw HelperError(message: "Missing output_path") }
        let edge = request["max_edge"] as? Int ?? 1600
        guard (1...8192).contains(edge) else { throw HelperError(message: "max_edge must be between 1 and 8192") }
        let image = try decode(source(path), edge: edge)
        guard let destination = CGImageDestinationCreateWithURL(URL(fileURLWithPath: output) as CFURL, UTType.png.identifier as CFString, 1, nil) else {
            throw HelperError(message: "Could not create PNG destination")
        }
        CGImageDestinationAddImage(destination, image, nil)
        guard CGImageDestinationFinalize(destination) else { throw HelperError(message: "Could not write PNG") }
        return ["width": image.width, "height": image.height]
    default: throw HelperError(message: "Unknown command: \(command)")
    }
}

while let line = readLine() {
    autoreleasepool {
        do {
            guard let data = line.data(using: .utf8),
                  let request = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
                throw HelperError(message: "Invalid JSON request")
            }
            let response: [String: Any] = ["ok": true, "result": try run(request)]
            let json = try JSONSerialization.data(withJSONObject: response, options: [.sortedKeys])
            print(String(decoding: json, as: UTF8.self))
        } catch {
            let response: [String: Any] = ["ok": false, "error": error.localizedDescription]
            let json = try! JSONSerialization.data(withJSONObject: response, options: [.sortedKeys])
            print(String(decoding: json, as: UTF8.self))
        }
        fflush(stdout)
    }
}
