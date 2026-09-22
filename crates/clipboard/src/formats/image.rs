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
    encoding: ImageEncoding,
    bytes: Arc<Vec<u8>>,
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
    /// The constructor bounds encoded storage; `validate` performs full decoding.
    pub fn encoding(&self) -> ImageEncoding {
        self.encoding
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    #[cfg(target_os = "linux")]
    pub(crate) fn shared_bytes(&self) -> Arc<Vec<u8>> {
        self.bytes.clone()
    }
    pub fn validate(&self) -> Result<()> {
        self.decode().map(|_| ())
    }
    /// Decode to owned RGBA pixels on a background worker. No third-party image
    /// types are part of this crate's public contract.
    pub fn rgba(&self) -> Result<(u32, u32, Vec<u8>)> {
        let image = self.decode()?.to_rgba8();
        Ok((image.width(), image.height(), image.into_raw()))
    }
    #[cfg(target_os = "windows")]
    pub(crate) fn from_native_bmp(bytes: Vec<u8>) -> Result<Self> {
        if bytes.is_empty() || bytes.len() > 256 * 1024 * 1024 + 138 {
            return Err(Error::InvalidData);
        }
        Ok(Self {
            encoding: ImageEncoding::Bmp,
            bytes: Arc::new(bytes),
        })
    }
    /// Decoding belongs on a background worker; bounds apply before allocating pixels.
    pub(crate) fn decode(&self) -> Result<DynamicImage> {
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
            .map_err(|_| Error::InvalidData)?;
        if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_PIXELS {
            return Err(Error::InvalidInput);
        }
        let mut reader = ImageReader::with_format(Cursor::new(self.bytes.as_slice()), format);
        let mut limits = Limits::default();
        limits.max_image_width = Some(64_000_000);
        limits.max_image_height = Some(64_000_000);
        limits.max_alloc = Some(256 * 1024 * 1024);
        reader.limits(limits);
        reader.decode().map_err(|_| Error::InvalidData)
    }
    pub fn png(&self) -> Result<Self> {
        let image = self.decode()?;
        encode(image)
    }
}
fn encode(image: DynamicImage) -> Result<ImageData> {
    let mut bytes = BoundedPng(Vec::new());
    image
        .write_with_encoder(image::codecs::png::PngEncoder::new(&mut bytes))
        .map_err(|e| Error::backend("encode clipboard image", e))?;
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
    fn image_roundtrip_and_invalid_encoding() {
        let original = ImageData::new(
            ImageEncoding::Png,
            include_bytes!("../../tests/fixtures/alpha.png").to_vec(),
        )
        .unwrap();
        assert_eq!(
            original.rgba().unwrap(),
            original.png().unwrap().rgba().unwrap()
        );
        assert!(
            ImageData::new(ImageEncoding::Png, vec![1, 2, 3])
                .unwrap()
                .validate()
                .is_err()
        );
    }
}
