use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};

use axum::{routing::get, Router};
use tower_ipfilter::{
    connection_info_service::AddConnectionInfoLayer, geo_filter::GeoIpv4Filter, ip_filter::{IpFilter, V4}, network_filter_service::FilterLayer
};
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing_subscriber::prelude::*;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                // axum logs rejections from built-in extractors with the `axum::rejection`
                // target, at `TRACE` level. `axum::rejection=trace` enables showing those events
                format!(
                    "{}=debug,tower_http=debug,axum::rejection=trace,maxmind_tower=debug",
                    env!("CARGO_CRATE_NAME")
                )
                .into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let geo_service = GeoIpv4Filter::new(
        tower_ipfilter::types::Mode::BlackList,
        "../../GeoLite2-Country-CSV_20241015.zip",
    )
    .unwrap();
    geo_service.set_countries(vec!["Norway".to_string(), "Sweden".to_string()]);

    let ip_service = IpFilter::<V4>::new(tower_ipfilter::types::Mode::BlackList);
    ip_service.add_ip(
        std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        "Test".to_string(),
        "2021-10-15".to_string(),
    ).await;

    let app = Router::new().route("/", get(handler)).layer(
        ServiceBuilder::new()
            .layer(TraceLayer::new_for_http())
            .layer(AddConnectionInfoLayer)
            .layer(FilterLayer::new(Arc::new(geo_service)))
            .into_inner(),
    );
    //.route_layer(from_extractor::<ExtractIp>());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .unwrap();

    tracing::info!("listening on http://{}", listener.local_addr().unwrap());
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

async fn handler() -> &'static str {
    //tracing::info!("Request from: {}", addr);
    "Hello, World!"
}
