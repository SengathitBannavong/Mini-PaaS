use crate::router::middleware::Middleware;

pub struct AddHeadersMiddleware {
    request_headers: Vec<(String, String)>,
    response_headers: Vec<(String, String)>,
}

impl AddHeadersMiddleware {
    pub fn new() -> Self {
        AddHeadersMiddleware {
            request_headers: Vec::new(),
            response_headers: Vec::new(),
        }
    }

    pub fn add_request_header(mut self, name: &str, value: &str) -> Self {
        self.request_headers
            .push((name.to_string(), value.to_string()));
        self
    }

    pub fn add_response_header(mut self, name: &str, value: &str) -> Self {
        self.response_headers
            .push((name.to_string(), value.to_string()));
        self
    }
}

impl Middleware for AddHeadersMiddleware {
    fn on_request(
        &self,
        mut req: hyper::Request<http_body_util::combinators::BoxBody<bytes::Bytes, hyper::Error>>,
        _ctx: &super::RequestContext,
    ) -> Result<
        hyper::Request<http_body_util::combinators::BoxBody<bytes::Bytes, hyper::Error>>,
        hyper::Response<http_body_util::combinators::BoxBody<bytes::Bytes, hyper::Error>>,
    > {
        for (name, value) in &self.request_headers {
            req.headers_mut().insert(
                hyper::header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                value.parse().unwrap(),
            );
        }
        Ok(req)
    }

    fn on_response(
        &self,
        mut resp: hyper::Response<http_body_util::combinators::BoxBody<bytes::Bytes, hyper::Error>>,
        _ctx: &super::RequestContext,
    ) -> hyper::Response<http_body_util::combinators::BoxBody<bytes::Bytes, hyper::Error>> {
        for (name, value) in &self.response_headers {
            resp.headers_mut().insert(
                hyper::header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                value.parse().unwrap(),
            );
        }
        resp
    }

    fn name(&self) -> &str {
        "add-headers"
    }
}
