use std::{
    net::IpAddr,
    task::{Context, Poll},
};
use http::Request;
use tower::{Layer, Service};

#[derive(Clone, Debug)]
pub struct AddConnectionInfo<S> {
    inner: S,
}

impl<S> AddConnectionInfo<S> {
    pub fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S, B> Service<Request<B>> for AddConnectionInfo<S>
where
    S: Service<Request<B>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<B>) -> Self::Future {
        if let Some(ip_addr) = extract_ip(&req) {
            req.extensions_mut().insert(ConnectionInfo { ip_addr });
        }
        self.inner.call(req)
    }
}


fn extract_ip<B>(req: &Request<B>) -> Option<IpAddr> {
    cfg_if::cfg_if! {
            if #[cfg(feature = "axum")] {
                use axum_impl::extract_ip_axum;
                return extract_ip_axum(&req)
            } else if #[cfg(feature = "hyper")] {
                use hyper_impl::extract_ip_hyper;
                return extract_ip_hyper(&req)
            } else {
                panic!("Either axum or hyper feature must be enabled")
            }
        };
}

#[derive(Clone, Copy, Debug)]
pub struct AddConnectionInfoLayer;

impl<S: Clone> Layer<S> for AddConnectionInfoLayer {
    type Service = AddConnectionInfo<S>;

    fn layer(&self, service: S) -> Self::Service {
        AddConnectionInfo::new(service)
    }
}

#[derive(Clone, Debug)]
pub struct ConnectionInfo {
    pub ip_addr: IpAddr,
}

#[cfg(feature = "axum")]
mod axum_impl {
    use super::*;
    use axum::extract::connect_info::ConnectInfo;
    use std::net::SocketAddr;

    pub fn extract_ip_axum<B>(req: &Request<B>) -> Option<IpAddr> {
        let headers_to_check = [
            "CF-Connecting-IP",
            "True-Client-IP",
            "X-Real-IP",
            "X-Forwarded-For",
        ];

        for header in headers_to_check.iter() {
            if let Some(ip) = req
                .headers()
                .get(*header)
                .and_then(|hv| hv.to_str().ok())
                .and_then(|s| s.split(',').next())
                .and_then(|s| s.trim().parse().ok())
            {
                return Some(ip);
            }
        }

        req.extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|socket_addr| socket_addr.ip())

    }
}

#[cfg(feature = "hyper")]
mod hyper_impl {
    use super::*;

    pub fn extract_ip_hyper<B>(req: &Request<B>) -> Option<IpAddr> {
        let headers_to_check = [
            "CF-Connecting-IP",
            "True-Client-IP",
            "X-Real-IP",
            "X-Forwarded-For",
        ];

        for header in headers_to_check.iter() {
            if let Some(ip) = req
                .headers()
                .get(*header)
                .and_then(|hv| hv.to_str().ok())
                .and_then(|s| s.split(',').next())
                .and_then(|s| s.trim().parse().ok())
            {
                return Some(ip);
            }
        }

        req.uri().host().and_then(|host| host.parse().ok())
    }
}

#[cfg(feature = "axum")]
pub use axum_impl::extract_ip_axum;

#[cfg(feature = "hyper")]
pub use hyper_impl::extract_ip_hyper;