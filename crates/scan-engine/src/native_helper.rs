//! Bridge to the bundled macOS ImageIO/Vision helper. Each request is one JSON line.
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use photo_core::AppError;
use serde_json::{json, Value};

fn executable() -> Result<PathBuf, AppError> {
    if let Some(path) = std::env::var_os("PHOTO_ANALYSIS_HELPER") {
        return Ok(path.into());
    }
    if let Ok(current) = std::env::current_exe() {
        if let Some(dir) = current.parent() {
            let bundled = dir.join("photo-analysis");
            if bundled.is_file() {
                return Ok(bundled);
            }
        }
    }
    // Development fallback; release builds use the adjacent bundled sidecar.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/binaries");
    let target = if cfg!(target_arch = "aarch64") {
        "aarch64-apple-darwin"
    } else {
        "x86_64-apple-darwin"
    };
    let path = root.join(format!("photo-analysis-{target}"));
    if path.is_file() {
        Ok(path)
    } else {
        Err(AppError::Image("ImageIO helper is unavailable".into()))
    }
}

const HELPER_TIMEOUT: Duration = Duration::from_secs(60);

fn request(request: Value) -> Result<Value, AppError> {
    request_with_executable(&executable()?, request, HELPER_TIMEOUT)
}

// Read stdout concurrently: feature prints can exceed the pipe buffer. A timeout
// also bounds an unresponsive decoder; cancellation is observed between photos.
fn request_with_executable(
    path: &Path,
    request: Value,
    timeout: Duration,
) -> Result<Value, AppError> {
    let mut child = Command::new(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| AppError::Image(format!("Could not start ImageIO helper: {e}")))?;
    let mut input = child
        .stdin
        .take()
        .ok_or_else(|| AppError::Image("ImageIO helper input unavailable".into()))?;
    let input_result = serde_json::to_writer(&mut input, &request)
        .map_err(|e| AppError::Image(e.to_string()))
        .and_then(|_| input.write_all(b"\n").map_err(AppError::Io));
    drop(input);
    if let Err(error) = input_result {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    let mut stdout = child.stdout.take().expect("piped helper stdout");
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout.read_to_end(&mut bytes).map(|_| bytes);
        let _ = sender.send(result);
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(AppError::Image("ImageIO helper timed out".into()));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(AppError::Io(error));
            }
        }
    };
    if !status.success() {
        return Err(AppError::Image(format!(
            "ImageIO helper exited with {status}"
        )));
    }
    let output = receiver
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(|_| AppError::Image("ImageIO helper output timed out".into()))?
        .map_err(AppError::Io)?;
    let response: Value = serde_json::from_slice(&output)
        .map_err(|e| AppError::Image(format!("Invalid ImageIO helper response: {e}")))?;
    if response.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(AppError::Image(
            response
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("ImageIO helper failed")
                .to_owned(),
        ));
    }
    response
        .get("result")
        .cloned()
        .ok_or_else(|| AppError::Image("ImageIO helper returned no result".into()))
}

pub fn capabilities() -> Result<Value, AppError> {
    request(json!({"command":"capabilities"}))
}
pub fn analyze(path: &Path) -> Result<Value, AppError> {
    request(json!({"command":"analyze", "path":path}))
}
pub fn dimensions(path: &Path) -> Result<(u32, u32), AppError> {
    let value = request(json!({"command":"dimensions", "path":path}))?;
    let width = value
        .get("width")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok());
    let height = value
        .get("height")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok());
    width
        .zip(height)
        .ok_or_else(|| AppError::Image("Invalid ImageIO dimensions".into()))
}
pub fn render(path: &Path, output: &Path, max_edge: u32) -> Result<Value, AppError> {
    request(json!({"command":"render", "path":path, "output_path":output, "max_edge":max_edge}))
}

/// Decode into a unique temporary PNG, cleaning it up on every return path.
pub fn decode(path: &Path, max_edge: u32) -> Result<image::DynamicImage, AppError> {
    let output = std::env::temp_dir().join(format!("photo-analysis-{}.png", uuid::Uuid::new_v4()));
    let result = (|| {
        render(path, &output, max_edge)?;
        image::open(&output).map_err(|e| AppError::Image(e.to_string()))
    })();
    let _ = std::fs::remove_file(output);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    fn fake_helper(script: &str) -> (tempfile::TempDir, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("helper");
        std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        (dir, path)
    }

    #[cfg(unix)]
    #[test]
    fn hung_helper_is_terminated() {
        let (_dir, path) = fake_helper("exec sleep 5");
        let start = Instant::now();
        let error = request_with_executable(
            &path,
            json!({"command":"analyze"}),
            Duration::from_millis(100),
        )
        .unwrap_err();
        assert!(error.to_string().contains("timed out"));
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[cfg(unix)]
    #[test]
    fn malformed_helper_response_is_reported() {
        let (_dir, path) = fake_helper("printf 'not-json'");
        let error = request_with_executable(
            &path,
            json!({"command":"capabilities"}),
            Duration::from_secs(1),
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("Invalid ImageIO helper response"));
    }

    // Opt-in because this requires a real HEIC fixture and macOS image services.
    #[test]
    fn heic_fixture_decodes_and_hashes_when_provided() {
        let Some(path) = std::env::var_os("PHOTO_ANALYSIS_TEST_HEIC") else {
            return;
        };
        let path = Path::new(&path);
        let (width, height) = dimensions(path).expect("HEIC dimensions");
        assert!(width > 0 && height > 0);
        let image = decode(path, 256).expect("HEIC render");
        assert!(image.width() > 0 && image.height() > 0);
        assert!(image.width() <= 256 && image.height() <= 256);
        crate::hash::perceptual_hashes(path).expect("HEIC hashes");
        let result = analyze(path).expect("HEIC Vision analysis");
        assert_eq!(result["width"].as_u64(), Some(u64::from(width)));
    }
}
