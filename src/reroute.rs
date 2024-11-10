use hyper::Uri;
use crate::config::config;

pub fn get_reroute(uri: &Uri) -> Option<String> {
    let reroute = &config().reroute.paths;
    let uri_string = uri.to_string();
    for route in reroute {
        if uri_string.starts_with(&route.from) {
            // swap in the new destination, keep the rest of the path
            return Some(route.to.to_string() + &uri_string[route.from.len()..]);
        }
    }

    // no reroute found, return original destination
    None
}