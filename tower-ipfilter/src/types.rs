use std::collections::HashMap;

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Encode, Decode, PartialEq)]
pub struct IpBlock {
    pub network: String,
    pub geoname_id: Option<u32>,
    pub registered_country_geoname_id: Option<u32>,
    pub represented_country_geoname_id: Option<u32>,
    #[serde(deserialize_with = "bool_deserialize")]
    pub is_anonymous_proxy: bool,
    #[serde(deserialize_with = "bool_deserialize")]
    pub is_satellite_provider: bool,
    #[serde(skip)]
    pub is_anycast: Option<bool>,
}

fn bool_deserialize<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    match s.as_str() {
        "1" => Ok(true),
        "0" => Ok(false),
        _ => Err(serde::de::Error::custom("invalid value")),
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Encode, Decode, PartialEq)]
pub struct CountryLocation {
    pub geoname_id: u32,
    pub locale_code: String,
    pub continent_code: String,
    pub continent_name: String,
    pub country_iso_code: Option<String>,
    pub country_name: Option<String>,
    #[serde(deserialize_with = "bool_deserialize")]
    pub is_in_european_union: bool,
}

#[derive(Serialize, Deserialize, Encode, Decode)]
pub struct GeoData {
    pub ip_blocks: Vec<IpBlock>,
    pub country_locations: HashMap<u32, CountryLocation>,
}

#[derive(Debug, Clone)]
pub enum Mode {
    BlackList,
    WhiteList,
}

impl Default for Mode {
    fn default() -> Self {
        Mode::BlackList
    }
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mode::BlackList => write!(f, "BlackList"),
            Mode::WhiteList => write!(f, "WhiteList"),
        }
    }
}

