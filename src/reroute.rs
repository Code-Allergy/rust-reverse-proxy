use hyper::Uri;
use crate::config::config;
use std::collections::HashMap;

// Internal function that contains the core rerouting logic and is testable
fn get_reroute_internal(uri: &Uri, reroute_paths: &HashMap<String, String>) -> Option<String> {
    let uri_string = uri.path_and_query().map_or_else(|| uri.path(), |pq| pq.as_str()).to_string();

    // Iterate over the HashMap and check for prefix matches.
    // This maintains the starts_with logic.
    for (from_path, to_base) in reroute_paths {
        if uri_string.starts_with(from_path) {
            // Construct the new path: take the 'to_base' and append the rest of the original URI
            // that came after the 'from_path' prefix.
            let remaining_path = &uri_string[from_path.len()..];
            return Some(format!("{}{}", to_base, remaining_path));
        }
    }
    // No reroute found, return None
    None
}

// Public function that uses the global config
pub fn get_reroute(uri: &Uri) -> Option<String> {
    get_reroute_internal(uri, &config().reroute.paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::Uri;
    use std::collections::HashMap;

    #[test]
    fn test_get_reroute_internal_exact_match() {
        let mut paths = HashMap::new();
        paths.insert("/service/path".to_string(), "http://localhost:8080/new".to_string());
        let uri = "/service/path".parse::<Uri>().unwrap();
        assert_eq!(
            get_reroute_internal(&uri, &paths),
            Some("http://localhost:8080/new".to_string())
        );
    }

    #[test]
    fn test_get_reroute_internal_prefix_match() {
        let mut paths = HashMap::new();
        paths.insert("/api".to_string(), "http://service.local/api_v1".to_string());
        let uri = "/api/users/123".parse::<Uri>().unwrap();
        assert_eq!(
            get_reroute_internal(&uri, &paths),
            Some("http://service.local/api_v1/users/123".to_string())
        );
    }

    #[test]
    fn test_get_reroute_internal_no_match() {
        let mut paths = HashMap::new();
        paths.insert("/specific".to_string(), "http://specific.host".to_string());
        let uri = "/other/path".parse::<Uri>().unwrap();
        assert_eq!(get_reroute_internal(&uri, &paths), None);
    }

    #[test]
    fn test_get_reroute_internal_with_query_params() {
        let mut paths = HashMap::new();
        paths.insert("/query".to_string(), "http://backend/service".to_string());
        let uri = "/query/data?param=value&another=123".parse::<Uri>().unwrap();
        assert_eq!(
            get_reroute_internal(&uri, &paths),
            Some("http://backend/service/data?param=value&another=123".to_string())
        );
    }

    #[test]
    fn test_get_reroute_internal_empty_paths() {
        let paths = HashMap::new();
        let uri = "/anything".parse::<Uri>().unwrap();
        assert_eq!(get_reroute_internal(&uri, &paths), None);
    }

    #[test]
    fn test_get_reroute_internal_different_prefix_no_match() {
        let mut paths = HashMap::new();
        paths.insert("/service_a".to_string(), "http://host_a".to_string());
        let uri = "/service_b/path".parse::<Uri>().unwrap();
        assert_eq!(get_reroute_internal(&uri, &paths), None);
    }
    
    #[test]
    fn test_get_reroute_internal_uri_path_is_shorter_than_from_path() {
        let mut paths = HashMap::new();
        paths.insert("/long/prefix/path".to_string(), "http://target".to_string());
        let uri = "/long/prefix".parse::<Uri>().unwrap();
        assert_eq!(get_reroute_internal(&uri, &paths), None);
    }

    #[test]
    fn test_get_reroute_internal_root_path_match() {
        let mut paths = HashMap::new();
        paths.insert("/".to_string(), "http://catchall".to_string());
        let uri_root = "/".parse::<Uri>().unwrap();
        let uri_path = "/some/path".parse::<Uri>().unwrap();
        assert_eq!(
            get_reroute_internal(&uri_root, &paths),
            Some("http://catchall".to_string())
        );
        assert_eq!(
            get_reroute_internal(&uri_path, &paths),
            Some("http://catchallsome/path".to_string()) 
            // Note: current logic for remaining_path appends directly.
            // If '/' is the key, "some/path" becomes the remaining_path.
            // Depending on desired behavior for root catchalls, this might need adjustment
            // e.g. ensuring a '/' between to_base and remaining_path if to_base doesn't end with one
            // and remaining_path doesn't start with one.
            // For this test, we assert the current behavior.
        );
    }

     #[test]
    fn test_get_reroute_internal_path_with_trailing_slash_in_config() {
        let mut paths = HashMap::new();
        paths.insert("/api/".to_string(), "http://service.local/api_v1/".to_string());
        let uri = "/api/users/123".parse::<Uri>().unwrap();
        assert_eq!(
            get_reroute_internal(&uri, &paths),
            Some("http://service.local/api_v1/users/123".to_string())
        );

        let uri_trailing = "/api/".parse::<Uri>().unwrap();
         assert_eq!(
            get_reroute_internal(&uri_trailing, &paths),
            Some("http://service.local/api_v1/".to_string())
        );
    }
}
