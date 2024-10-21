use dashmap::DashMap;
use ipnetwork::{IpNetwork, Ipv4Network, Ipv6Network};
use tracing::info;

use crate::{
    compress::{load_compressed_data, save_compressed_data},
    extract::extract_and_parse_csv,
    network_filter_service::NetworkFilter,
    types::{CountryLocation, Mode},
    IpServiceTrait,
};
use std::{
    error::Error,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    path::{Path, PathBuf},
};

pub trait IpAddrExt: Sized + Send {
    fn to_network(self) -> IpNetwork;
    fn to_ip_addr(self) -> IpAddr;
    fn is_ipv4(&self) -> bool;
}

impl IpAddrExt for Ipv4Addr {
    fn to_ip_addr(self) -> IpAddr {
        IpAddr::V4(self)
    }
    fn to_network(self) -> IpNetwork {
        IpNetwork::V4(Ipv4Network::from(self))
    }
    fn is_ipv4(&self) -> bool {
        true
    }
}

impl IpAddrExt for Ipv6Addr {
    fn to_ip_addr(self) -> IpAddr {
        IpAddr::V6(self)
    }
    fn to_network(self) -> IpNetwork {
        IpNetwork::V6(Ipv6Network::from(self))
    }
    fn is_ipv4(&self) -> bool {
        false
    }
}

impl IpAddrExt for IpAddr {
    fn to_ip_addr(self) -> IpAddr {
        self
    }
    fn to_network(self) -> IpNetwork {
        match self {
            IpAddr::V4(ip) => IpNetwork::V4(Ipv4Network::from(ip)),
            IpAddr::V6(ip) => IpNetwork::V6(Ipv6Network::from(ip)),
        }
    }
    fn is_ipv4(&self) -> bool {
        match self {
            IpAddr::V4(_) => true,
            IpAddr::V6(_) => false,
        }
    }
}

impl IpAddrExt for Ipv4Network {
    fn to_ip_addr(self) -> IpAddr {
        IpAddr::V4(self.network())
    }
    fn to_network(self) -> IpNetwork {
        IpNetwork::V4(self)
    }
    fn is_ipv4(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone)]
pub struct GeoIpv4Filter {
    pub networks: DashMap<Ipv4Network, CountryLocation>,
    pub addresses: DashMap<Ipv4Addr, CountryLocation>,
    pub countries: DashMap<String, bool>,
    pub mode: Mode,
}

impl GeoIpv4Filter {
    pub fn new(mode: Mode, path_to_data: impl Into<PathBuf>) -> Result<Self, Box<dyn Error>> {
        let data_path = Path::new("geo_ip_data.bin.gz");

        let geo_data = if !data_path.exists() {
            let data = extract_and_parse_csv(&path_to_data.into())?;
            save_compressed_data(&data, data_path)?;
            data
        } else {
            load_compressed_data(data_path)?
        };

        info!(
            "Loaded {} ip blocks and {} country locations",
            geo_data.ip_blocks.len(),
            geo_data.country_locations.len()
        );

        let ip_country_map = DashMap::<Ipv4Network, CountryLocation>::new();

        // add localhost
        ip_country_map.insert(
            Ipv4Network::from(Ipv4Addr::new(127, 0, 0, 1)),
            CountryLocation {
                geoname_id: 0,
                locale_code: "NB".to_string(),
                continent_code: "NA".to_string(),
                continent_name: "Europe".to_string(),
                country_iso_code: Some("NO".to_string()),
                country_name: Some("Norway".to_string()),
                is_in_european_union: true,
            },
        );

        for block in geo_data.ip_blocks {
            if let Some(geoname_id) = block.geoname_id {
                if let Ok(network) = block.network.parse() {
                    if let Some(country) = geo_data.country_locations.get(&geoname_id) {
                        ip_country_map.insert(network, country.clone());
                    } else {
                        println!("No country found for geoname_id: {}", geoname_id);
                    }
                }
            }
        }

        Ok(Self {
            networks: ip_country_map,
            addresses: DashMap::new(),
            countries: DashMap::new(),
            mode,
        })
    }

    pub async fn get_country_for_ip(&self, ip: &Ipv4Addr) -> Option<CountryLocation> {
        let mut country = None;

        if let Some(location) = self.addresses.get(ip) {
            return Some(location.clone());
        }

        for kv in self.networks.iter() {
            let (network, location) = kv.pair();
            if network.contains(*ip) {
                country = Some(location.clone());
                break;
            }
        }
        country
    }

    pub async fn add_ip(&self, ip: Ipv4Addr, reason: String, date: String) {
        if let Some(country) = self.get_country_for_ip(&ip).await {
            self.addresses.insert(ip, country.clone());
        }
    }

    pub fn remove_ip(&self, ip: Ipv4Addr) {
        self.addresses.remove(&ip);
    }

    pub async fn add_network(&self, network: Ipv4Network, reason: String, date: String) {
        if let Some(country) = self.get_country_for_ip(&network.network()).await {
            self.networks.insert(network, country.clone());
        }
    }

    pub fn remove_network(&self, network: Ipv4Network) {
        self.networks.remove(&network);
    }

    pub fn set_countries(&self, countries: Vec<String>) {
        self.countries.clear();
        for country in countries {
            self.countries.insert(country, true);
        }
    }

    pub async fn is_country_blocked(&self, country: &str) -> bool {
        match self.mode {
            Mode::BlackList => self.countries.contains_key(country),
            Mode::WhiteList => !self.countries.contains_key(country),
        }
    }

    pub async fn is_ip_blocked(&self, ip: &Ipv4Addr) -> bool {
        if let Some(country) = self.get_country_for_ip(ip).await {
            let name = country.country_name.unwrap();
            let is_blocked = self.is_country_blocked(&name).await;
            tracing::info!("{} is blocked: {}", is_blocked, name);
            is_blocked
        } else {
            false
        }
    }
}

impl NetworkFilter for GeoIpv4Filter {
    fn block(
        &self,
        ip: impl IpAddrExt,
        network: bool,
    ) -> impl std::future::Future<Output = ()> + Send {
        async move {
            if network {
                match ip.to_network() {
                    IpNetwork::V4(ip) => {
                        self.add_network(ip, "Blocked".to_string(), "2021-01-01".to_string())
                            .await;
                    }
                    _ => {}
                }
            } else {
                match ip.to_ip_addr() {
                    IpAddr::V4(ip) => {
                        self.add_ip(ip, "Blocked".to_string(), "2021-01-01".to_string())
                            .await;
                    }
                    _ => {}
                }
            }
        }
    }

    fn unblock(
        &self,
        ip: impl IpAddrExt,
        network: bool,
    ) -> impl std::future::Future<Output = ()> + Send {
        async move {
            if network {
                match ip.to_network() {
                    IpNetwork::V4(ip) => {
                        self.remove_network(ip);
                    }
                    _ => {}
                }
            } else {
                match ip.to_ip_addr() {
                    IpAddr::V4(ip) => {
                        self.remove_ip(ip);
                    }
                    _ => {}
                }
            }
        }
    }

    fn is_blocked(&self, ip: impl IpAddrExt) -> impl std::future::Future<Output = bool> + Send {
        async move {
            match ip.to_ip_addr() {
                IpAddr::V4(ip) => !self.is_ip_blocked(&ip).await,
                _ => false,
            }
        }
    }
}
