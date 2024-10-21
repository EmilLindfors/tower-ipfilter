use std::io::BufReader;
use std::{error::Error, fs::File, io::BufWriter, path::Path};
use flate2::write::GzEncoder;
use flate2::read::GzDecoder;
use flate2::Compression;
use crate::types::GeoData;

const BINCODE_CONFIG : bincode::config::Configuration = bincode::config::standard();

pub fn save_compressed_data(data: &GeoData, path: &Path) -> Result<(), Box<dyn Error>> {
    let file = File::create(path)?;
    let encoder = GzEncoder::new(file, Compression::default());
    let mut writer = BufWriter::new(encoder);
    
    bincode::encode_into_std_write(data, &mut writer, BINCODE_CONFIG)?;
    Ok(())
}

pub fn load_compressed_data(path: &Path) -> Result<GeoData, Box<dyn Error>> {
    let file = File::open(path)?;
    let decoder = GzDecoder::new(file);
    let reader = BufReader::new(decoder);
    let data: GeoData = bincode::decode_from_reader(reader, BINCODE_CONFIG)?;
    Ok(data)
}