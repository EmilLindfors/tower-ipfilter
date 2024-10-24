use bytes::Bytes;
use http::{HeaderValue, Response, StatusCode};
use http_body::{Body, SizeHint};
use http_body_util::Full;
use pin_project_lite::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project! {
    pub struct IpResponseBody<B> {
        #[pin]
        inner: IpResponseBodyInner<B>
    }
}

impl<B> IpResponseBody<B> {
    fn geo_access_denied() -> Self {
        Self {
            inner: IpResponseBodyInner::AccessDenied {
                body: Full::from(ACCESS_DENIED_GEO_BODY),
            },
        }
    }

    fn ip_address_denied() -> Self {
        Self {
            inner: IpResponseBodyInner::AccessDenied {
                body: Full::from(ACCESS_DENIED_IP_BODY),
            },
        }
    }

    fn ip_not_found() -> Self {
        Self {
            inner: IpResponseBodyInner::AccessDenied {
                body: Full::from(ACCESS_DENIED_NOT_FOUND_BODY),
            },
        }
    }

    pub(crate) fn new(body: B) -> Self {
        Self {
            inner: IpResponseBodyInner::Body { body },
        }
    }
}

pin_project! {
    #[project = BodyProj]
    enum IpResponseBodyInner<B> {
        AccessDenied {
            #[pin]
            body: Full<Bytes>,
        },
        Body {
            #[pin]
            body: B
        }
    }
}

impl<B> Body for IpResponseBody<B>
where
    B: Body<Data = Bytes>,
{
    type Data = Bytes;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        match self.project().inner.project() {
            BodyProj::AccessDenied { body } => body.poll_frame(cx).map_err(|err| match err {}),
            BodyProj::Body { body } => body.poll_frame(cx),
        }
    }

    fn is_end_stream(&self) -> bool {
        match &self.inner {
            IpResponseBodyInner::AccessDenied { body } => body.is_end_stream(),
            IpResponseBodyInner::Body { body } => body.is_end_stream(),
        }
    }

    fn size_hint(&self) -> SizeHint {
        match &self.inner {
            IpResponseBodyInner::AccessDenied { body } => body.size_hint(),
            IpResponseBodyInner::Body { body } => body.size_hint(),
        }
    }
}

const ACCESS_DENIED_GEO_BODY: &[u8] = b"Access denied based on country of origin";
const ACCESS_DENIED_IP_BODY: &[u8] = b"Access denied based on IP address";
const ACCESS_DENIED_NOT_FOUND_BODY: &[u8] = b"Access denied IP not found";

pub fn create_geo_access_denied_response<B>() -> Response<IpResponseBody<B>>
where
    B: Body,
{
    let mut res = Response::new(IpResponseBody::geo_access_denied());
    *res.status_mut() = StatusCode::FORBIDDEN;
    res.headers_mut().insert(
        http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    res
}

pub fn create_ip_not_found_response<B>() -> Response<IpResponseBody<B>>
where
    B: Body,
{
    let mut res = Response::new(IpResponseBody::ip_not_found());
    *res.status_mut() = StatusCode::FORBIDDEN;
    res.headers_mut().insert(
        http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    res
}

pub fn create_ip_address_denied_response<B>() -> Response<IpResponseBody<B>>
where
    B: Body,
{
    let mut res = Response::new(IpResponseBody::ip_address_denied());
    *res.status_mut() = StatusCode::FORBIDDEN;
    res.headers_mut().insert(
        http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    res
}
