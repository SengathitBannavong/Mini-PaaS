use super::{Middleware, RequestContext};
use crate::ratelimit::{RateLimitResult, RateLimiter};
use bytes::Bytes;
use http_body_util::{BodyExt, Full, combinators::BoxBody};
use hyper::{Request, Response};

pub struct RateLimitMiddleware {
    limiter: RateLimiter,
}

impl RateLimitMiddleware {
    pub fn new(limiter: RateLimiter) -> Self {
        RateLimitMiddleware { limiter }
    }
}

impl Middleware for RateLimitMiddleware {
    fn on_request(
        &self,
        req: Request<BoxBody<Bytes, hyper::Error>>,
        ctx: &RequestContext,
    ) -> Result<Request<BoxBody<Bytes, hyper::Error>>, Response<BoxBody<Bytes, hyper::Error>>> {
        match self.limiter.check(&ctx.client_ip) {
            RateLimitResult::Allowed => {
                // Request is allowed - we'll add rate limit headers in on_response
                Ok(req)
            }
            RateLimitResult::Limited { retry_after_secs } => {
                // Too many requests - short-circuit with 429
                let body = Full::new(Bytes::from(
					"429 Too Many Requests\nYou have exceeded the rate limit. Please try again later.\n",
					))
					.map_err(|never| match never {})
					.boxed();

                let response = Response::builder()
                    .status(429)
                    .header("Retry-After", retry_after_secs.to_string())
                    .header("Content-Type", "text/plain")
                    .body(body)
                    .unwrap();
                Err(response)
            }
        }
    }

    fn on_response(
        &self,
        mut resp: Response<BoxBody<Bytes, hyper::Error>>,
        ctx: &RequestContext,
    ) -> Response<BoxBody<Bytes, hyper::Error>> {
        // Add rate limit info headers to successful responses
        let headers = resp.headers_mut();
        headers.insert("X-RateLimit-Limit", self.limiter.limit().into());
        headers.insert(
            "X-RateLimit-Remaining",
            self.limiter.peek(&ctx.client_ip).into(),
        );
        resp
    }

    fn name(&self) -> &str {
        "rate-limit"
    }
}
