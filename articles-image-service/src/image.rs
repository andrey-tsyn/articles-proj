use std::fs::File;
use std::io::{ErrorKind};
use std::path::Path;
use image::{ColorType, DynamicImage, EncodableLayout, ImageEncoder};
use image::codecs::jpeg::JpegEncoder;
use crate::common::StringPath;

pub fn save_as_jpeg(res_path: &Path, image: &DynamicImage, quality: u8) -> Result<StringPath, anyhow::Error> {
    let encoder =
        JpegEncoder::new_with_quality(File::create(&res_path).unwrap(), quality);

    encoder.write_image(image.to_rgb8().as_bytes(),
                              image.width(), image.height(),
                              ColorType::Rgb8)?;

    Ok(StringPath::from(res_path.to_str().unwrap()))
}