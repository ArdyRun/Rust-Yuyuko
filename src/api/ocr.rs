// OCR module using owocr (Python) for Japanese text detection from images
// Calls owocr's Google Lens engine via subprocess

use anyhow::Result;
use tracing::{debug, warn};

/// Run OCR on image data using owocr's Google Lens engine
/// Saves image to temp file, runs Python subprocess, returns detected text
pub async fn ocr_image(image_data: &[u8], mime_type: &str) -> Result<String> {
    let ext = match mime_type {
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "jpg",
    };

    // Write image to temp file
    let temp_path = std::env::temp_dir().join(format!("ayumi_ocr_{}.{}", std::process::id(), ext));
    tokio::fs::write(&temp_path, image_data).await?;

    debug!("OCR: saved temp image to {:?}", temp_path);

    // Run owocr via Python subprocess from venv
    let python_path = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("venv")
        .join("Scripts")
        .join("python.exe");

    let result = tokio::process::Command::new(python_path)
        .args([
            "-c",
            &format!(
                r#"
import sys
import asyncio
from pathlib import Path
from owocr.ocr import GoogleLens

async def run_ocr():
    try:
        engine = GoogleLens()
        res = engine(Path(r"{}"))
        success, ocr_result = res
        
        if success and ocr_result:
            text_lines = []
            for p in ocr_result.paragraphs:
                for l in p.lines:
                    if getattr(l, 'text', None):
                        text_lines.append(l.text)
                    else:
                        text_lines.append(''.join(w.text for w in l.words))
            
            final_text = '\n'.join(text_lines).strip()
            print(final_text)
        else:
            print("")
    except Exception as e:
        print(f"OCR_ERROR: {{e}}", file=sys.stderr)
        sys.exit(1)

asyncio.run(run_ocr())
"#,
                temp_path.to_string_lossy()
            ),
        ])
        .output()
        .await;

    // Cleanup temp file
    let _ = tokio::fs::remove_file(&temp_path).await;

    match result {
        Ok(output) => {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if text.is_empty() {
                    Ok("テキストが検出されませんでした。(Tidak ada teks terdeteksi)".to_string())
                } else {
                    debug!("OCR detected text: {} chars", text.len());
                    Ok(text)
                }
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("owocr failed: {}", stderr);
                anyhow::bail!("owocr error: {}", stderr.trim())
            }
        }
        Err(e) => {
            warn!("Failed to run owocr subprocess: {:?}", e);
            anyhow::bail!("owocr not available: {:?}", e)
        }
    }
}
