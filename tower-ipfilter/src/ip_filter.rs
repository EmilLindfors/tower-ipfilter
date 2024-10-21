use std::{
    marker::PhantomData,
    net::IpAddr,
};

use dashmap::DashMap;
use ipnetwork::IpNetwork;

use crate::{geo_filter::IpAddrExt, network_filter_service::NetworkFilter, types::Mode};

#[derive(Debug, Clone)]
pub struct IpMetaData {
    pub reason: String,
    pub date: String,
}

#[derive(Debug, Clone)]
pub enum V4 {}

#[derive(Debug, Clone)]
pub enum V6 {}

pub trait IpType {}

impl IpType for V4 {}
impl IpType for V6 {}

#[derive(Debug, Clone)]
pub struct IpFilter<S: IpType> {
    pub addresses: DashMap<IpAddr, IpMetaData>,
    pub networks: DashMap<IpNetwork, IpMetaData>,
    pub mode: Mode,
    marker: PhantomData<S>,
}

impl<S: IpType> IpFilter<S> {
    pub fn new(mode: Mode) -> Self {
        Self {
            networks: DashMap::new(),
            addresses: DashMap::new(),
            mode,
            marker: PhantomData,
        }
    }
    pub async fn add_ip(&self, ip: IpAddr, reason: String, date: String) {
        self.addresses.insert(ip, IpMetaData { reason, date });
    }
    pub async fn add_network(&self, network: IpNetwork, reason: String, date: String) {
        self.networks.insert(network, IpMetaData { reason, date });
    }

    async fn is_ip_blocked(&self, ip: &IpAddr) -> bool {
        if self.addresses.contains_key(ip) {
            match self.mode {
                Mode::BlackList => return true,
                Mode::WhiteList => return false,
            }
        } else {
            for kv in self.networks.iter() {
                let (network, _) = kv.pair();
                if network.contains(*ip) {
                    match self.mode {
                        Mode::BlackList => return true,
                        Mode::WhiteList => return false,
                    }
                }
            }

            match self.mode {
                Mode::BlackList => return false,
                Mode::WhiteList => return true,
            }
        }
    }

    async fn block_ip(&self, ip: impl IpAddrExt, network: bool) {
        if network {
            match ip.to_network() {
                IpNetwork::V4(ip) => {
                    self.add_network(
                        IpNetwork::V4(ip),
                        "Blocked".to_string(),
                        "2021-09-01".to_string(),
                    )
                    .await;
                }
                IpNetwork::V6(ip) => {
                    self.add_network(
                        IpNetwork::V6(ip),
                        "Blocked".to_string(),
                        "2021-09-01".to_string(),
                    )
                    .await;
                }
            }
        } else {
            match ip.to_ip_addr() {
                IpAddr::V4(ip) => {
                    self.add_ip(
                        IpAddr::V4(ip),
                        "Blocked".to_string(),
                        "2021-09-01".to_string(),
                    )
                    .await;
                }
                IpAddr::V6(ip) => {
                    self.add_ip(
                        IpAddr::V6(ip),
                        "Blocked".to_string(),
                        "2021-09-01".to_string(),
                    )
                    .await;
                }
            }
        }
    }

    async fn unblock_ip(&self, ip: impl IpAddrExt, network: bool) {
        if network {
            match ip.to_network() {
                IpNetwork::V4(ip) => {
                    self.networks.remove(&IpNetwork::V4(ip));
                }
                IpNetwork::V6(ip) => {
                    self.networks.remove(&IpNetwork::V6(ip));
                }
            }
        } else {
            match ip.to_ip_addr() {
                IpAddr::V4(ip) => {
                    self.addresses.remove(&IpAddr::V4(ip));
                }
                IpAddr::V6(ip) => {
                    self.addresses.remove(&IpAddr::V6(ip));
                }
            }
        }
    }
}

impl NetworkFilter for IpFilter<V4> {
    fn block(
        &self,
        ip: impl IpAddrExt,
        network: bool,
    ) -> impl std::future::Future<Output = ()> + Send {
        async move {
            if ip.is_ipv4() {
                self.block_ip(ip, network).await;
            } else {
                panic!("Invalid IP address");
            }
        }
    }

    fn unblock(
        &self,
        ip: impl IpAddrExt,
        network: bool,
    ) -> impl std::future::Future<Output = ()> + Send {
        async move {
            if ip.is_ipv4() {
                self.unblock_ip(ip, network).await;
            } else {
                panic!("Invalid IP address");
            }
        }
    }

    fn is_blocked(&self, ip: impl IpAddrExt) -> impl std::future::Future<Output = bool> + Send {
        async move {
            if ip.is_ipv4() {
                self.is_ip_blocked(&ip.to_ip_addr()).await
            } else {
                panic!("Invalid IP address");
            }
        }
    }
}

impl NetworkFilter for IpFilter<V6> {
  fn block(
      &self,
      ip: impl IpAddrExt,
      network: bool,
  ) -> impl std::future::Future<Output = ()> + Send {
      async move {
          if !ip.is_ipv4() {
              self.block_ip(ip, network).await;
          } else {
              panic!("Invalid IP address");
          }
      }
  }

  fn unblock(
      &self,
      ip: impl IpAddrExt,
      network: bool,
  ) -> impl std::future::Future<Output = ()> + Send {
      async move {
          if !ip.is_ipv4() {
              self.unblock_ip(ip, network).await;
          } else {
              panic!("Invalid IP address");
          }
      }
  }

  fn is_blocked(&self, ip: impl IpAddrExt) -> impl std::future::Future<Output = bool> + Send {
      async move {
          if !ip.is_ipv4() {
              self.is_ip_blocked(&ip.to_ip_addr()).await
          } else {
              panic!("Invalid IP address");
          }
      }
  }
}
