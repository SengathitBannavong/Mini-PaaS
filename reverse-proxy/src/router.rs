use std::sync::atomic::AtomicUsize;

use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::Ordering;

#[derive(Debug, Clone)]
pub struct Backend {
    pub host: String,
    pub port: u16,
    pub url: String,
    pub healthy: bool,
}

impl Backend {
    pub fn new(url: &str) -> Self {
        let trimmed = url.trim_start_matches("http://");
        let parts: Vec<&str> = trimmed.split(':').collect();
        let host = parts[0].to_string();
        let port: u16 = parts.get(1).unwrap_or(&"80").parse().unwrap_or(80);
        Backend {
            host,
            port,
            url: url.to_string(),
            healthy: true,
        }
    }

    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[derive(Debug)]
pub struct Route {
    pub path_prefix: String,
    backends: RwLock<Vec<Backend>>,
    current: AtomicUsize,
}

impl Route {
    pub fn new(path_prefix: &str, backend_url: &Vec<&str>) -> Self {
        let backends: Vec<Backend> = backend_url.iter().map(|url| Backend::new(url)).collect();
        Route {
            path_prefix: path_prefix.to_string(),
            backends: RwLock::new(backends),
            current: AtomicUsize::new(0),
        }
    }

    // Check if a given path matches the route's prefix
    pub fn matches(&self, path: &str) -> bool {
        path.starts_with(&self.path_prefix)
    }

    /// Get the next healthy backend using round-robin
    pub fn next_backend(&self) -> Option<Backend> {
        let backends = self.backends.read().unwrap();
        let healthy: Vec<&Backend> = backends.iter().filter(|b| b.healthy).collect();

        if healthy.is_empty() {
            return None;
        }

        let idx = self.current.fetch_add(1, Ordering::Relaxed) % healthy.len();
        Some(healthy[idx].clone())
    }

    /// Get all backends (for health checking)
    pub fn get_backends(&self) -> Vec<Backend> {
        self.backends.read().unwrap().clone()
    }

    /// Update the health status of a backend
    pub fn set_backend_health(&self, url: &str, healthy: bool) {
        let mut backends = self.backends.write().unwrap();
        if let Some(backend) = backends.iter_mut().find(|b| b.url == url) {
            if backend.healthy != healthy {
                let status = if healthy { "healthy" } else { "unhealthy" };
                println!("  [health] {} is now {}", url, status);
            }
            backend.healthy = healthy;
        }
    }
}

pub struct Router {
    routes: Vec<Arc<Route>>,
}

impl Router {
    pub fn new() -> Self {
        Router { routes: Vec::new() }
    }

    // Add a new route to the router
    pub fn add_route(&mut self, path_prefix: &str, backend_urls: Vec<&str>) {
        let route = Arc::new(Route::new(path_prefix, &backend_urls));
        let backends: Vec<String> = backend_urls.iter().map(|u| u.to_string()).collect();
        println!(" Route added: {} -> {}", path_prefix, backends.join(", "));
        self.routes.push(route);
    }

    // Find the route that matches the given path
    // Returns the longest matching prefix (most specific route wins)
    pub fn find_route(&self, path: &str) -> Option<Arc<Route>> {
        self.routes
            .iter()
            .filter(|route| route.matches(path))
            .max_by_key(|route| route.path_prefix.len())
            .cloned()
    }

    pub fn all_routes(&self) -> &[Arc<Route>] {
        &self.routes
    }
}
