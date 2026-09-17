use crate::{Error, Result};
use image::{DynamicImage, ImageFormat, ImageReader, Limits};
use std::{
    io::{self, Cursor, Write},
    sync::Arc,
};

pub const MAX_IMAGE_BYTES: usize = 64 * 1024 * 1024;
const MAX_PIXELS: u64 = 64_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageEncoding {
    Png,
    Jpeg,
    Tiff,
    Bmp,
}
#[derive(Clone, PartialEq, Eq)]
pub struct ImageData {
    pub encoding: ImageEncoding,
    pub bytes: Arc<Vec<u8>>,
}
impl std::fmt::Debug for ImageData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageData")
            .field("encoding", &self.encoding)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}
impl ImageData {
    pub fn new(encoding: ImageEncoding, bytes: Vec<u8>) -> Result<Self> {
        if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
            return Err(Error::InvalidInput);
        }
        Ok(Self {
            encoding,
            bytes: Arc::new(bytes),
        })
    }
    /// Decoding belongs on a background worker; bounds apply before allocating pixels.
    pub fn decode(&self) -> Result<DynamicImage> {
        let limit = if self.encoding == ImageEncoding::Bmp {
            256 * 1024 * 1024 + 138
        } else {
            MAX_IMAGE_BYTES
        };
        if self.bytes.len() > limit {
            return Err(Error::InvalidInput);
        }
        let format = match self.encoding {
            ImageEncoding::Png => ImageFormat::Png,
            ImageEncoding::Jpeg => ImageFormat::Jpeg,
            ImageEncoding::Tiff => ImageFormat::Tiff,
            ImageEncoding::Bmp => ImageFormat::Bmp,
        };
        let (width, height) = ImageReader::with_format(Cursor::new(self.bytes.as_slice()), format)
            .into_dimensions()
            .map_err(|_| Error::InvalidInput)?;
        if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_PIXELS {
            return Err(Error::InvalidInput);
        }
        let mut reader = ImageReader::with_format(Cursor::new(self.bytes.as_slice()), format);
        let mut limits = Limits::default();
        limits.max_image_width = Some(64_000_000);
        limits.max_image_height = Some(64_000_000);
        limits.max_alloc = Some(256 * 1024 * 1024);
        reader.limits(limits);
        reader.decode().map_err(|_| Error::InvalidInput)
    }
    pub fn png(&self) -> Result<Self> {
        let image = self.decode()?;
        encode(image)
    }
    pub fn thumbnail(&self) -> Result<(Vec<u8>, u32, u32)> {
        let image = self.decode()?;
        let size = (image.width(), image.height());
        let preview = encode(image.thumbnail(384, 256))?;
        Ok((preview.bytes.as_ref().clone(), size.0, size.1))
    }
    #[cfg(target_os = "windows")]
    pub(crate) fn dib_v5(&self) -> Result<Vec<u8>> {
        let pixels = self.decode()?.to_rgba8();
        let mut bytes = vec![0u8; 124 + pixels.len()];
        bytes[0..4].copy_from_slice(&124u32.to_le_bytes());
        bytes[4..8].copy_from_slice(&(pixels.width() as i32).to_le_bytes());
        bytes[8..12].copy_from_slice(&(-(pixels.height() as i32)).to_le_bytes());
        bytes[12..14].copy_from_slice(&1u16.to_le_bytes());
        bytes[14..16].copy_from_slice(&32u16.to_le_bytes());
        bytes[16..20].copy_from_slice(&3u32.to_le_bytes());
        bytes[20..24].copy_from_slice(&(pixels.len() as u32).to_le_bytes());
        for (offset, mask) in [
            (40, 0x00ff0000u32),
            (44, 0x0000ff00),
            (48, 0x000000ff),
            (52, 0xff000000),
            (56, 0x73524742),
        ] {
            bytes[offset..offset + 4].copy_from_slice(&mask.to_le_bytes());
        }
        for (input, output) in pixels
            .as_raw()
            .chunks_exact(4)
            .zip(bytes[124..].chunks_exact_mut(4))
        {
            output.copy_from_slice(&[input[2], input[1], input[0], input[3]]);
        }
        Ok(bytes)
    }
    #[cfg(target_os = "windows")]
    pub(crate) fn from_dib(dib: &[u8]) -> Result<Self> {
        if dib.len() < 40 || dib.len() > 256 * 1024 * 1024 + 124 {
            return Err(Error::InvalidInput);
        }
        let u32_at = |p| u32::from_le_bytes(dib[p..p + 4].try_into().unwrap());
        let header = u32_at(0) as usize;
        if ![40, 52, 56, 108, 124].contains(&header) || header > dib.len() {
            return Err(Error::InvalidInput);
        }
        let bits = u16::from_le_bytes(dib[14..16].try_into().unwrap());
        let compression = u32_at(16);
        let colors = if u32_at(32) != 0 {
            u32_at(32) as usize
        } else if bits <= 8 {
            1usize << bits
        } else {
            0
        };
        let masks = if header == 40 && compression == 3 {
            12
        } else if header == 40 && compression == 6 {
            16
        } else {
            0
        };
        let offset = header
            .checked_add(masks)
            .and_then(|n| colors.checked_mul(4).and_then(|c| n.checked_add(c)))
            .ok_or(Error::InvalidInput)?;
        if offset > dib.len() {
            return Err(Error::InvalidInput);
        }
        let mut bmp = Vec::with_capacity(dib.len() + 14);
        bmp.extend_from_slice(b"BM");
        bmp.extend_from_slice(&((dib.len() + 14) as u32).to_le_bytes());
        bmp.extend_from_slice(&[0; 4]);
        bmp.extend_from_slice(&((offset + 14) as u32).to_le_bytes());
        bmp.extend_from_slice(dib);
        // DIB can be larger than the encoded-image bound; decode with pixel limits, then compress.
        let raw = Self {
            encoding: ImageEncoding::Bmp,
            bytes: Arc::new(bmp),
        };
        Ok(raw)
    }
}
fn encode(image: DynamicImage) -> Result<ImageData> {
    let mut bytes = BoundedPng(Vec::new());
    image
        .write_with_encoder(image::codecs::png::PngEncoder::new(&mut bytes))
        .map_err(|_| Error::Native)?;
    ImageData::new(ImageEncoding::Png, bytes.0)
}
struct BoundedPng(Vec<u8>);
impl Write for BoundedPng {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_IMAGE_BYTES - self.0.len() {
            return Err(io::Error::other("encoded image exceeds limit"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn png_roundtrip_preserves_transparent_pixels_and_thumbnail_bounds() {
        let image = DynamicImage::ImageRgba8(image::RgbaImage::from_fn(900, 450, |x, _| {
            image::Rgba([12, 30, 80, (x % 256) as u8])
        }));
        let encoded = encode(image.clone()).unwrap();
        assert_eq!(
            encoded.png().unwrap().decode().unwrap().to_rgba8(),
            image.to_rgba8()
        );
        let (preview, width, height) = encoded.thumbnail().unwrap();
        assert_eq!((width, height), (900, 450));
        let thumbnail = ImageData::new(ImageEncoding::Png, preview)
            .unwrap()
            .decode()
            .unwrap();
        assert!(thumbnail.width() <= 384 && thumbnail.height() <= 256);
        assert!(
            ImageData::new(ImageEncoding::Png, vec![1, 2, 3])
                .unwrap()
                .decode()
                .is_err()
        );
    }
}
