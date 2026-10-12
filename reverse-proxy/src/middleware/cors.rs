use bytes::Bytes;
use http_body_util::{BodyExt, Full, combinators::BoxBody};
use hyper::Response;

use crate::middleware::Middleware;

pub struct CorsMiddleware {
    allowed_origins: Vec<String>,
    allowed_methods: String,
    allowed_headers: String,
}

impl CorsMiddleware {
    pub fn new(allowed_origins: Vec<String>) -> Self {
        CorsMiddleware {
            allowed_origins,
            allowed_methods: "GET, POST, PUT, DELETE, OPTIONS".to_string(),
            allowed_headers: "Content-Type, Authorization".to_string(),
        }
    }

    fn match_origin(&self, origin: &str) -> Option<String> {
        if self.allowed_origins.iter().any(|o| o == "*") {
            Some("*".into())
        } else if self.allowed_origins.iter().any(|o| o == origin) {
            Some(origin.into())
        } else {
            None
        }
    }
}

impl Middleware for CorsMiddleware {
    fn on_request(
        &self,
        req: hyper::Request<http_body_util::combinators::BoxBody<bytes::Bytes, hyper::Error>>,
        ctx: &super::RequestContext,
    ) -> Result<
        hyper::Request<http_body_util::combinators::BoxBody<bytes::Bytes, hyper::Error>>,
        hyper::Response<http_body_util::combinators::BoxBody<bytes::Bytes, hyper::Error>>,
    > {
        // Handle preflight OPTIONS request
        if req.method() == hyper::Method::OPTIONS {
            let Some(origin) = ctx.origin.as_deref() else {
                return Ok(req); // not a CORS request, pass through
            };
            let Some(allowed_origin) = self.match_origin(origin) else {
                let body = Full::new(Bytes::new())
                    .map_err(|never| match never {})
                    .boxed();
                return Err(Response::builder().status(403).body(body).unwrap());
            };

            let body = Full::new(Bytes::new())
                .map_err(|never| match never {})
                .boxed();

            let response = Response::builder()
                .status(204)
                .header("Access-Control-Allow-Origin", &allowed_origin)
                .header("Access-Control-Allow-Methods", &self.allowed_methods)
                .header("Access-Control-Allow-Headers", &self.allowed_headers)
                .header("Access-Control-Max-Age", "86400")
                .body(body)
                .unwrap();

            return Err(response); // Short-circult: don't forward to backend
        }
        Ok(req)
    }

    fn on_response(
        &self,
        mut resp: Response<BoxBody<Bytes, hyper::Error>>,
        ctx: &super::RequestContext,
    ) -> Response<BoxBody<Bytes, hyper::Error>> {
        // Echo back the request's origin only if it's allowed
        if let Some(o) = ctx.origin.as_deref().and_then(|o| self.match_origin(o)) {
            let headers = resp.headers_mut();
            headers.insert("Access-Control-Allow-Origin", o.parse().unwrap());
            headers.insert("Vary", "Origin".parse().unwrap());
        }
        resp
    }

    fn name(&self) -> &str {
        "cors"
    }
}
