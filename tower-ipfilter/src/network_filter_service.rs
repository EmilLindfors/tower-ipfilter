use crate::{
    body::{create_access_denied_response, create_ip_not_found_response, GeoIpResponseBody}, connection_info_service::ConnectionInfo, geo_filter::IpAddrExt, IpServiceTrait
};
use bytes::Bytes;
use futures_lite::FutureExt;
use http::{Request, Response};
use http_body::Body;
use ipnetwork::IpNetwork;
use pin_project_lite::pin_project;
use std::{future::Future, sync::Arc};
use std::{
    net::IpAddr,
    task::{Context, Poll},
};
use tower_service::Service;
use tracing::debug;

pub trait NetworkFilter: Send + Sync + 'static {
    fn block(&self, ip: impl IpAddrExt, network: bool) -> impl Future<Output = ()> + Send;
    fn unblock(&self, ip: impl IpAddrExt, network: bool) -> impl Future<Output = ()> + Send;
    fn is_blocked(&self, ip: impl IpAddrExt) -> impl Future<Output = bool> + Send;
}

#[derive(Clone)]
// Generic Filter service
pub struct Filter<S, F> {
    inner: S,
    filter: Arc<F>,
}

impl<S, F> Filter<S, F>
where
    F: NetworkFilter,
{
    pub fn new(inner: S, filter: Arc<F>) -> Self {
        Self { inner, filter }
    }

    pub fn layer(filter: Arc<F>) -> FilterLayer<F> {
        FilterLayer { filter }
    }
}

#[derive(Clone)]
pub struct FilterLayer<F> {
    filter: Arc<F>,
}

impl<F> FilterLayer<F>
where
    F: NetworkFilter,
{
    pub fn new(filter: Arc<F>) -> Self {
        Self { filter }
    }
}

impl<S, F> tower_layer::Layer<S> for FilterLayer<F>
where
    F: NetworkFilter,
{
    type Service = Filter<S, F>;

    fn layer(&self, inner: S) -> Self::Service {
        Filter::new(inner, self.filter.clone())
    }
}




impl<S: Clone, ReqBody, ResBody, F: NetworkFilter + 'static> Service<Request<ReqBody>>
    for Filter<S, F>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Send + 'static,
    S::Future: Send + 'static,
    ReqBody: Send + 'static,
    ResBody: Body<Data = Bytes> + Send + 'static,
    ResBody::Error: std::error::Error + Send + Sync + 'static,
{
    type Response = Response<GeoIpResponseBody<ResBody>>;
    type Error = S::Error;
    type Future = futures_lite::future::Boxed<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let geo_service = self.filter.clone();
        let inner = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, inner);

        async move {
            if let Some(ip) = req
                .extensions()
                .get::<ConnectionInfo>()
                .map(|socket_addr| socket_addr.ip_addr)
            {
                if geo_service.is_blocked(ip).await {
                    return Ok(create_access_denied_response());
                 
                } else {
                    return inner
                    .call(req)
                    .await
                    .map(|res| res.map(GeoIpResponseBody::new));
                }
            } else {
                tracing::warn!("No IP address found in request, blocking request");
                return Ok(create_ip_not_found_response());
            }
        }
        .boxed()
    }
}

pub fn filter<F: NetworkFilter>(filter: F) -> FilterLayer<F> {
    FilterLayer::new(Arc::new(filter))
}

#[cfg(test)]
mod tests {
    use crate::{geo_filter::GeoIpv4Filter, ip_filter::IpFilter, types::CountryLocation};

    use super::*;

    use axum::{
        body::Body,
        http::{Request, StatusCode},
        response::IntoResponse,
        routing::get,
        Router,
    };
    use dashmap::DashMap;
    use ipnetwork::{IpNetwork, Ipv4Network};
    use std::{net::SocketAddr, str::FromStr};
    use tower::{Layer, ServiceExt};
    use tower_http::trace::TraceLayer;

    async fn handler() -> impl IntoResponse {
        "Hello, World!"
    }

    async fn test_request(app: Router, request: Request<Body>) -> StatusCode {
        app.oneshot(request).await.unwrap().status()
    }

    fn create_test_geo_ip_service() -> GeoIpv4Filter {
        let ip_country_map = DashMap::new();

        ip_country_map.insert(
            Ipv4Network::from_str("192.168.0.0/16").unwrap(),
            CountryLocation {
                geoname_id: 1,
                locale_code: "EN".to_string(),
                continent_code: "EU".to_string(),
                continent_name: "Europe".to_string(),
                country_iso_code: Some("GB".to_string()),
                country_name: Some("United Kingdom".to_string()),
                is_in_european_union: false,
            },
        );

        ip_country_map.insert(
            Ipv4Network::from_str("10.0.0.0/8").unwrap(),
            CountryLocation {
                geoname_id: 2,
                locale_code: "EN".to_string(),
                continent_code: "NA".to_string(),
                continent_name: "North America".to_string(),
                country_iso_code: Some("US".to_string()),
                country_name: Some("United States".to_string()),
                is_in_european_union: false,
            },
        );

        GeoIpv4Filter {
            networks: ip_country_map,
            addresses: DashMap::new(),
            countries: DashMap::new(),
            mode: Default::default(),
        }
    }

    fn create_app(geo_service: GeoIpv4Filter) -> Router {
        Router::new()
            .route("/", get(handler))
            .layer(TraceLayer::new_for_http())
            .layer(filter(geo_service))
    }

    #[tokio::test]
    async fn test_geo_ip_filter_allowed_country() {
        let geo_service = create_test_geo_ip_service();
        geo_service.set_countries(vec!["United States".to_string()]);
        let app = create_app(geo_service);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("X-Forwarded-For", "192.168.1.1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_geo_ip_filter_blocked_country() {
        let geo_service = create_test_geo_ip_service();
        geo_service.set_countries(vec!["United States".to_string()]);
        let app = create_app(geo_service);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("X-Forwarded-For", "10.0.0.1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_geo_ip_filter_unknown_ip() {
        let geo_service = create_test_geo_ip_service();
        geo_service.set_countries(vec!["United States".to_string()]);
        let app = create_app(geo_service);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("X-Forwarded-For", "172.16.0.1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_geo_ip_filter_no_ip_header() {
        let geo_service = create_test_geo_ip_service();
        geo_service.set_countries(vec!["United States".to_string()]);
        let app = create_app(geo_service);

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_geo_ip_filter_x_forwarded_for() {
        let geo_service = create_test_geo_ip_service();
        geo_service.set_countries(vec!["United States".to_string()]);
        let app = create_app(geo_service);

        let allowed_request = Request::builder()
            .uri("/")
            .header("X-Forwarded-For", "192.168.1.1")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            test_request(app.clone(), allowed_request).await,
            StatusCode::OK
        );

        let blocked_request = Request::builder()
            .uri("/")
            .header("X-Forwarded-For", "10.0.0.1")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            test_request(app.clone(), blocked_request).await,
            StatusCode::FORBIDDEN
        );
    }

    #[tokio::test]
    async fn test_geo_ip_filter_x_real_ip() {
        let geo_service = create_test_geo_ip_service();
        geo_service.set_countries(vec!["United States".to_string()]);
        let app = create_app(geo_service);

        let allowed_request = Request::builder()
            .uri("/")
            .header("X-Real-IP", "192.168.1.1")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            test_request(app.clone(), allowed_request).await,
            StatusCode::OK
        );

        let blocked_request = Request::builder()
            .uri("/")
            .header("X-Real-IP", "10.0.0.1")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            test_request(app.clone(), blocked_request).await,
            StatusCode::FORBIDDEN
        );
    }

    #[tokio::test]
    async fn test_geo_ip_filter_cf_connecting_ip() {
        let geo_service = create_test_geo_ip_service();
        geo_service.set_countries(vec!["United States".to_string()]);
        let app = create_app(geo_service);

        let allowed_request = Request::builder()
            .uri("/")
            .header("CF-Connecting-IP", "192.168.1.1")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            test_request(app.clone(), allowed_request).await,
            StatusCode::OK
        );

        let blocked_request = Request::builder()
            .uri("/")
            .header("CF-Connecting-IP", "10.0.0.1")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            test_request(app.clone(), blocked_request).await,
            StatusCode::FORBIDDEN
        );
    }

    #[tokio::test]
    async fn test_geo_ip_filter_true_client_ip() {
        let geo_service = create_test_geo_ip_service();
        geo_service.set_countries(vec!["United States".to_string()]);
        let app = create_app(geo_service);

        let allowed_request = Request::builder()
            .uri("/")
            .header("True-Client-IP", "192.168.1.1")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            test_request(app.clone(), allowed_request).await,
            StatusCode::OK
        );

        let blocked_request = Request::builder()
            .uri("/")
            .header("True-Client-IP", "10.0.0.1")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            test_request(app.clone(), blocked_request).await,
            StatusCode::FORBIDDEN
        );
    }

    #[tokio::test]
    async fn test_geo_ip_filter_connection_info() {
        let geo_service = create_test_geo_ip_service();
        geo_service.set_countries(vec!["United States".to_string()]);
        let app = create_app(geo_service);

        let allowed_request = Request::builder()
            .uri("/")
            .extension(SocketAddr::from_str("192.168.1.1:12345").unwrap())
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            test_request(app.clone(), allowed_request).await,
            StatusCode::OK
        );

        let blocked_request = Request::builder()
            .uri("/")
            .extension(SocketAddr::from_str("10.0.0.1:12345").unwrap())
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            test_request(app.clone(), blocked_request).await,
            StatusCode::FORBIDDEN
        );
    }

    #[tokio::test]
    async fn test_geo_ip_filter_multiple_headers() {
        let geo_service = create_test_geo_ip_service();
        geo_service.set_countries(vec!["United States".to_string()]);
        let app = create_app(geo_service);

        // Test with multiple headers, expecting the first valid IP to be used
        let request = Request::builder()
            .uri("/")
            .header("X-Forwarded-For", "172.16.0.1, 192.168.1.1")
            .header("X-Real-IP", "10.0.0.1")
            .header("CF-Connecting-IP", "203.0.113.1") // This should be used
            .body(Body::empty())
            .unwrap();
        assert_eq!(test_request(app.clone(), request).await, StatusCode::OK);

        // Test with multiple headers, but the most preferred one is blocked
        let request = Request::builder()
            .uri("/")
            .header("X-Forwarded-For", "172.16.0.1, 192.168.1.1")
            .header("X-Real-IP", "192.168.1.1")
            .header("CF-Connecting-IP", "10.0.0.1") // This should be used and blocked
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            test_request(app.clone(), request).await,
            StatusCode::FORBIDDEN
        );
    }

    #[tokio::test]
    async fn test_geo_ip_filter_no_ip() {
        let geo_service = create_test_geo_ip_service();
        geo_service.set_countries(vec!["United States".to_string()]);
        let app = create_app(geo_service);

        let request = Request::builder().uri("/").body(Body::empty()).unwrap();
        assert_eq!(test_request(app.clone(), request).await, StatusCode::OK);
    }
}
