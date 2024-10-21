use std::net::IpAddr;

pub mod types;
mod compress;
mod extract;
mod body;
pub mod geo_filter;
pub mod ip_filter;
pub mod network_filter_service;
pub mod connection_info_service;



pub trait IpServiceTrait: Send + Sync {
    async fn add_ip(&self, ip: IpAddr, reason: String, date: String);
    fn is_ip_blocked(&self, ip: &IpAddr) -> impl std::future::Future<Output = bool> + Send;
}
#[cfg(test)]
mod tests {
    use dashmap::DashMap;
    use geo_filter::GeoIpv4Filter;
    use ipnetwork::{IpNetwork, Ipv4Network};
    use types::{CountryLocation};

    use super::*;
    use std::net::{IpAddr, Ipv4Addr};
    use std::str::FromStr;

    fn create_test_geo_ip_service() -> GeoIpv4Filter {
        let ip_networks = DashMap::new();

        // Add some test data
        ip_networks.insert(Ipv4Network::from_str("192.168.0.0/16").unwrap(), CountryLocation {
            geoname_id: 1,
            locale_code: "EN".to_string(),
            continent_code: "EU".to_string(),
            continent_name: "Europe".to_string(),
            country_iso_code: Some("GB".to_string()),
            country_name: Some("United Kingdom".to_string()),
            is_in_european_union: false,
        });
        ip_networks.insert(Ipv4Network::from_str("10.0.0.0/8").unwrap(), CountryLocation {
            geoname_id: 2,
            locale_code: "EN".to_string(),
            continent_code: "NA".to_string(),
            continent_name: "North America".to_string(),
            country_iso_code: Some("US".to_string()),
            country_name: Some("United States".to_string()),
            is_in_european_union: false,
        });
        ip_networks.insert(Ipv4Network::from_str("172.16.0.0/12").unwrap(),  CountryLocation {
            geoname_id: 3,
            locale_code: "FR".to_string(),
            continent_code: "EU".to_string(),
            continent_name: "Europe".to_string(),
            country_iso_code: Some("FR".to_string()),
            country_name: Some("France".to_string()),
            is_in_european_union: true,
        });
        //ip_networks.insert(Ipv4Network::from_str("2001:db8::/32").unwrap(), CountryLocation {
        //    geoname_id: 4,
        //    locale_code: "JA".to_string(),
        //    continent_code: "AS".to_string(),
        //    continent_name: "Asia".to_string(),
        //    country_iso_code: Some("JP".to_string()),
        //    country_name: Some("Japan".to_string()),
        //    is_in_european_union: false,
        //});


        GeoIpv4Filter {
            networks: ip_networks,
            addresses: DashMap::new(),
            countries: DashMap::new(),
            mode: Default::default(),
        }
    }

    #[tokio::test]
    async fn test_get_country_for_ip() {
        let service = create_test_geo_ip_service();

        // Test IPv4 addresses
        assert_eq!(
            service.get_country_for_ip(&Ipv4Addr::from_str("192.168.1.1").unwrap()).await.unwrap().country_name,
            Some("United Kingdom".to_string())
        );
        assert_eq!(
            service.get_country_for_ip(&Ipv4Addr::from_str("10.0.0.1").unwrap()).await.unwrap().country_name,
            Some("United States".to_string())
        );
        assert_eq!(
            service.get_country_for_ip(&Ipv4Addr::from_str("172.16.0.1").unwrap()).await.unwrap().country_name,
            Some("France".to_string())
        );

        // Test IPv6 address
        //assert_eq!(
        //    service.get_country_for_ip(&Ipv4Addr::from_str("2001:db8::1").unwrap()).await.unwrap().country_name,
        //    Some("Japan".to_string())
        //);
//
        //// Test IP address not in any network
        //assert_eq!(
        //    service.get_country_for_ip(&Ipv4Addr::from_str("8.8.8.8").unwrap()).await,
        //    None
        //);
    }

    #[tokio::test]
    async fn test_get_country_for_ip_edge_cases() {
        let service = create_test_geo_ip_service();

        // Test edge of network
        assert_eq!(
            service.get_country_for_ip(&Ipv4Addr::from_str("192.168.255.255").unwrap()).await.unwrap().country_name,
            Some("United Kingdom".to_string())
        );

        // Test start of network
        assert_eq!(
            service.get_country_for_ip(&Ipv4Addr::from_str("10.0.0.0").unwrap()).await.unwrap().country_name,
            Some("United States".to_string())
        );

        // Test end of network
        assert_eq!(
            service.get_country_for_ip(&Ipv4Addr::from_str("10.255.255.255").unwrap()).await.unwrap().country_name,
            Some("United States".to_string())
        );
    }

    #[tokio::test]
    
    async fn test_blocklist() {
        let service = create_test_geo_ip_service();

        // Set up blocklist
        service.set_countries(vec!["United States".to_string(), "France".to_string()]);

        // Test blocked countries
        assert!(service.is_country_blocked("United States").await);
        assert!(service.is_country_blocked("France").await);
        assert!(!service.is_country_blocked("United Kingdom").await);
        assert!(!service.is_country_blocked("Japan").await);

        // Test blocked IPs
        assert!(service.is_ip_blocked(&Ipv4Addr::from_str("10.0.0.1").unwrap()).await); // US
        assert!(service.is_ip_blocked(&Ipv4Addr::from_str("172.16.0.1").unwrap()).await); // France
        assert!(!service.is_ip_blocked(&Ipv4Addr::from_str("192.168.1.1").unwrap()).await); // UK
        //assert!(!service.is_ip_blocked(&Ipv4Addr::from_str("2001:db8::1").unwrap()).await); // Japan

        // Test IP not in any network
        assert!(!service.is_ip_blocked(&Ipv4Addr::from_str("8.8.8.8").unwrap()).await);

        // Update blocklist
        service.set_countries(vec!["Japan".to_string()]);

        // Test updated blocklist
        assert!(!service.is_ip_blocked(&Ipv4Addr::from_str("10.0.0.1").unwrap()).await); // US
        //assert!(service.is_ip_blocked(&Ipv4Addr::from_str("2001:db8::1").unwrap()).await); // Japan
    }
}
