//! Bound provider output before decoding; allow same-aspect output to fit the original canvas.
pub(crate) const MAX_PIXELS: u64 = 16_777_216;
#[cfg(any(test, not(target_arch = "wasm32")))]
pub(crate) const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;

#[cfg(any(test, not(target_arch = "wasm32")))]
pub(crate) fn dimensions(bytes: &[u8], canvas_width: u32, canvas_height: u32) -> Result<(u32, u32), &'static str> {
    let error = "AI result must be a PNG under 64 MiB and 16 MP with the canvas aspect ratio; no layer was added";
    let dim = |offset: usize| bytes.get(offset..offset + 4).and_then(|s| <[u8; 4]>::try_from(s).ok()).map(u32::from_be_bytes);
    if bytes.len() > MAX_FILE_BYTES || bytes.get(..8) != Some(b"\x89PNG\r\n\x1a\n") || bytes.get(12..16) != Some(b"IHDR") {
        return Err(error);
    }
    let (width, height) = (dim(16).ok_or(error)?, dim(20).ok_or(error)?);
    if width == 0 || height == 0 || canvas_width == 0 || canvas_height == 0 || u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(error);
    }
    let a = u64::from(width) * u64::from(canvas_height);
    let b = u64::from(height) * u64::from(canvas_width);
    // Model dimensions may round by a pixel, but a materially different crop must not stretch.
    if a.abs_diff(b).saturating_mul(100) > a.max(b) {
        return Err(error);
    }
    Ok((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn header(w: u32, h: u32) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        bytes.extend_from_slice(&w.to_be_bytes());
        bytes.extend_from_slice(&h.to_be_bytes());
        bytes
    }
    #[test]
    fn accepts_original_dimensions() {
        assert_eq!(dimensions(&header(1024, 1024), 1024, 1024), Ok((1024, 1024)));
    }
    #[test]
    fn accepts_real_codex_size_and_same_aspect_resize() {
        assert_eq!(dimensions(&header(1254, 1254), 1024, 1024), Ok((1254, 1254)));
        assert_eq!(dimensions(&header(1536, 1024), 1200, 800), Ok((1536, 1024)));
        assert!(dimensions(&header(1536, 1023), 1200, 800).is_ok());
    }
    #[test]
    fn refuses_changed_aspect_ratio() {
        assert!(dimensions(&header(1024, 1536), 1024, 1024).is_err());
    }
    #[test]
    fn refuses_malformed_zero_and_oversized_results() {
        assert!(dimensions(b"bad", 8, 6).is_err());
        for (w, h) in [(0, 6), (8, 0), (u32::MAX, u32::MAX), (5000, 5000)] {
            assert!(dimensions(&header(w, h), 8, 6).is_err());
        }
        assert!(dimensions(&header(8, 6), 0, 0).is_err());
    }
}
