//! PNG-only asset and screenshot IO. No resizing or color-management engine.
use std::io::{BufRead, BufReader, BufWriter, Seek};
use std::path::Path;

pub struct Image {
    width: u32,
    height: u32,
    pub data: Vec<u8>,
}

impl Image {
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
    pub fn pixels(&self) -> impl Iterator<Item = &[u8; 4]> {
        self.data.as_chunks::<4>().0.iter()
    }
    pub fn get_pixel(&self, x: u32, y: u32) -> [u8; 4] {
        assert!(x < self.width && y < self.height);
        self.data.as_chunks::<4>().0[(y * self.width + x) as usize]
    }
    pub fn enumerate_pixels(&self) -> impl Iterator<Item = (u32, u32, &[u8; 4])> {
        self.pixels()
            .enumerate()
            .map(|(i, p)| (i as u32 % self.width, i as u32 / self.width, p))
    }
}

pub fn load(path: impl AsRef<Path>) -> Result<Image, String> {
    decode(BufReader::new(
        std::fs::File::open(path).map_err(|e| e.to_string())?,
    ))
}

fn decode(input: impl BufRead + Seek) -> Result<Image, String> {
    let mut decoder = png::Decoder::new(input);
    decoder.set_limits(png::Limits {
        bytes: 512 * 1024 * 1024,
    });
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let info = reader.info();
    let rgba_size = (info.width as usize)
        .checked_mul(info.height as usize)
        .and_then(|n| n.checked_mul(4))
        .filter(|n| *n <= 512 * 1024 * 1024)
        .ok_or("PNG exceeds the 512 MiB decoded image limit")?;
    let size = reader
        .output_buffer_size()
        .filter(|n| *n <= 512 * 1024 * 1024)
        .ok_or("PNG output buffer is too large")?;
    let mut raw = vec![0; size];
    let output = reader.next_frame(&mut raw).map_err(|e| e.to_string())?;
    let mut data = Vec::with_capacity(rgba_size);
    for pixel in raw[..output.buffer_size()].chunks_exact(output.color_type.samples()) {
        match output.color_type {
            png::ColorType::Rgba => data.extend_from_slice(pixel),
            png::ColorType::Rgb => data.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]),
            png::ColorType::Grayscale => {
                data.extend_from_slice(&[pixel[0], pixel[0], pixel[0], 255])
            }
            png::ColorType::GrayscaleAlpha => {
                data.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]])
            }
            png::ColorType::Indexed => return Err("PNG palette was not expanded".into()),
        }
    }
    Ok(Image {
        width: output.width,
        height: output.height,
        data,
    })
}

pub fn save(path: impl AsRef<Path>, data: &[u8], width: u32, height: u32) -> Result<(), String> {
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4));
    if expected != Some(data.len()) || width == 0 || height == 0 {
        return Err("Invalid RGBA image dimensions".into());
    }
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(data).map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expands_asset_color_formats_and_preserves_alpha() {
        for (color, input, expected) in [
            (png::ColorType::Rgb, vec![10, 20, 30], [10, 20, 30, 255]),
            (png::ColorType::Rgba, vec![10, 20, 30, 40], [10, 20, 30, 40]),
            (png::ColorType::Grayscale, vec![70], [70, 70, 70, 255]),
            (
                png::ColorType::GrayscaleAlpha,
                vec![70, 25],
                [70, 70, 70, 25],
            ),
            (png::ColorType::Indexed, vec![0], [10, 20, 30, 40]),
        ] {
            let mut bytes = Vec::new();
            let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
            encoder.set_color(color);
            encoder.set_depth(png::BitDepth::Eight);
            if color == png::ColorType::Indexed {
                encoder.set_palette(vec![10, 20, 30]);
                encoder.set_trns(vec![40]);
            }
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&input).unwrap();
            writer.finish().unwrap();
            let image = decode(std::io::Cursor::new(bytes)).unwrap();
            assert_eq!(image.get_pixel(0, 0), expected);
        }
    }
    #[test]
    fn rejects_invalid_png_data() {
        assert!(decode(std::io::Cursor::new(b"not a png")).is_err());
    }
}
