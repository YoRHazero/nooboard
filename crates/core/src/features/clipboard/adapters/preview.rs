//! Application preview sizing and encoding, independent of native clipboard access.
use nooboard_clipboard::{Error, ImageData, Result};

pub(crate) fn thumbnail(image: &ImageData) -> Result<(Vec<u8>, u32, u32)> {
    use image::ImageEncoder;
    let (width, height, pixels) = image.rgba()?;
    let image = image::RgbaImage::from_raw(width, height, pixels).ok_or(Error::InvalidData)?;
    let preview = image::DynamicImage::ImageRgba8(image)
        .thumbnail(384, 256)
        .to_rgba8();
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(
            preview.as_raw(),
            preview.width(),
            preview.height(),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|_| Error::InvalidData)?;
    Ok((bytes, width, height))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_keeps_original_dimensions_and_fits_ui_bounds() {
        use image::ImageEncoder;
        let pixels = image::RgbaImage::from_pixel(900, 450, image::Rgba([12, 30, 80, 90]));
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(pixels.as_raw(), 900, 450, image::ExtendedColorType::Rgba8)
            .unwrap();
        let image = ImageData::new(nooboard_clipboard::ImageEncoding::Png, bytes).unwrap();
        let (preview, width, height) = thumbnail(&image).unwrap();
        assert_eq!((width, height), (900, 450));
        let (w, h, rgba) = ImageData::new(nooboard_clipboard::ImageEncoding::Png, preview)
            .unwrap()
            .rgba()
            .unwrap();
        assert!(w <= 384 && h <= 256);
        assert!(rgba.chunks_exact(4).all(|pixel| pixel[3] == 90));
    }
}
