#[derive(Debug, Clone)]
pub struct Route {
    pub path_prefix: String,
    pub backend_url: String,
    pub backend_host: String,
    pub backend_port: u16,
}

impl Route {
    pub fn new(path_prefix: &str, backend_url: &str) -> Self {
        // Parse the backend Url to extract host and port
        let url = backend_url.trim_start_matches("http://");
        let parts: Vec<&str> = url.split(':').collect();
        let host = parts[0].to_string();
        let port: u16 = parts.get(1).unwrap_or(&"80").parse().unwrap_or(80);

        Route {
            path_prefix: path_prefix.to_string(),
            backend_url: backend_url.to_string(),
            backend_host: host,
            backend_port: port,
        }
    }

    // Check if a given path matches the route's prefix
    pub fn matches(&self, path: &str) -> bool {
        path.starts_with(&self.path_prefix)
    }
}

pub struct Router {
    routes: Vec<Route>,
}

impl Router {
    pub fn new() -> Self {
        Router { routes: Vec::new() }
    }

    // Add a new route to the router
    pub fn add_route(&mut self, path_prefix: &str, backend_url: &str) {
        let route = Route::new(path_prefix, backend_url);
        self.routes.push(route);
        println!(" Route added: {} -> {}", path_prefix, backend_url);
    }

    // Find the route that matches the given path
    // Returns the longest matching prefix (most specific route wins)
    pub fn find_route(&self, path: &str) -> Option<&Route> {
        self.routes
            .iter()
            .filter(|route| route.matches(path))
            .max_by_key(|route| route.path_prefix.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_matches() {
        let route = Route::new("/api/users", "http://127.0.0.1:8081");
        assert!(route.matches("/api/users"));
        assert!(route.matches("/api/users/123"));
        assert!(!route.matches("/api/orders"));
        assert!(!route.matches("/health"));
    }

    #[test]
    fn test_router_finds_most_specific_route() {
        let mut router = Router::new();
        router.add_route("/api", "http://127.0.0.1:8080");
        router.add_route("/api/users", "http://127.0.0.1:8081");

        let route = router.find_route("/api/users/123").unwrap();
        assert_eq!(route.backend_port, 8081);
    }

    #[test]
    fn test_router_returns_none_for_no_match() {
        let router = Router::new();
        assert!(router.find_route("/anything").is_none());
    }

    #[test]
    fn test_fallback_route() {
        let mut router = Router::new();
        router.add_route("/api/users", "http://127.0.0.1:8081");
        router.add_route("/", "http://127.0.0.1:8080");

        // Specific route
        let route = router.find_route("/api/users/1").unwrap();
        assert_eq!(route.backend_port, 8081);

        // Fallback to /
        let route = router.find_route("/health").unwrap();
        assert_eq!(route.backend_port, 8080);
    }
}
