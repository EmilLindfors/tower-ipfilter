use crate::types::{CountryLocation, GeoData, IpBlock};
use std::collections::HashMap;
use std::io::BufReader;
use std::{error::Error, fs::File, path::Path};

pub fn extract_and_parse_csv(path_to_data: &Path ) -> Result<GeoData, Box<dyn Error>> {
 
    let file = File::open(path_to_data)?;
    let mut archive = zip::ZipArchive::new(BufReader::new(file))?;

    let mut ip_blocks = Vec::new();
    {
        let ipv4_file =
            archive.by_name("GeoLite2-Country-CSV_20241015/GeoLite2-Country-Blocks-IPv4.csv")?;
        let mut rdr = csv::Reader::from_reader(ipv4_file);
        for result in rdr.deserialize() {
            let record: IpBlock = result?;
            ip_blocks.push(record);
        }
    }

    let mut country_locations = HashMap::new();
    let locations_file =
        archive.by_name("GeoLite2-Country-CSV_20241015/GeoLite2-Country-Locations-en.csv")?;
    let mut rdr = csv::Reader::from_reader(locations_file);
    for result in rdr.deserialize() {
        let record: CountryLocation = result?;
        country_locations.insert(record.geoname_id, record);
    }

    Ok(GeoData {
        ip_blocks,
        country_locations,
    })
}
