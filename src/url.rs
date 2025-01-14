pub struct Url {
    pub query: String,
    pub scheme: String,
    pub path: String,
    pub fragment: String,
    pub hostname: String,
    pub port: String,
}

impl Url {
    pub fn new(url: &str) -> Self {
        let mut base: &str;
        let mut query = "";
        let mut scheme = "";
        let mut path = String::new();
        let mut fragment = "";
        let hostname: &str;
        let mut port = "";

        // split off fragment
        let result = url.split_once("#");
        if !result.is_none() {
            let (a, b) = result.unwrap();
            base = a;
            fragment = b;
        } else {
            base = url;
        }

        // split off query
        let result = base.split_once("?");
        if !result.is_none() {
            let (a, b) = result.unwrap();
            base = a;
            query = b;
        }

        // split off scheme
        let result = base.split_once("://");
        if !result.is_none() {
            let (a, b) = result.unwrap();
            scheme = a;
            base = b;
        }

        // split off path
        let result = base.split_once("/");
        if !result.is_none() {
            let (a, b) = result.unwrap();
            base = a;
            path = format!("/{}", b);
        }

        // split off port
        let result = base.split_once(":");
        if !result.is_none() {
            let (a, b) = result.unwrap();
            base = a;
            port = b;
        }

        hostname = base;

        return Url {
            query: query.to_string(),
            scheme: scheme.to_string(),
            path: path,
            fragment: fragment.to_string(),
            hostname: hostname.to_string(),
            port: port.to_string(),
        };
    }

    pub fn to_string(&self) -> String {
        let query: String;
        let scheme: String;
        let mut fragment = String::new();
        let mut port = String::new();

        if self.scheme.len() > 0 {
            scheme = format!("{}://", self.scheme);
        } else {
            scheme = self.scheme.clone();
        }

        if self.port.len() > 0 {
            port = format!(":{}", self.port);
        }

        if self.query.len() > 0 {
            query = format!("?{}", self.query);
        } else {
            query = self.query.clone();
        }

        if self.fragment.len() > 0 {
            fragment = format!("#{}", self.fragment);
        }
        return format!(
            "{}{}{}{}{}{}",
            scheme, self.hostname, port, self.path, query, fragment
        );
    }

    pub fn host(&self) -> String {
        let mut port = String::new();

        if self.port.len() > 0 {
            port = format!(":{}", self.port);
        }

        return format!("{}{}", self.hostname, port);
    }

    pub fn resource(&self) -> String {
        let path: String;
        let query: String;

        if self.path.len() > 0 {
            path = format!("{}", self.query);
        } else {
            path = "/".to_string();
        }

        if self.query.len() > 0 {
            query = format!("?{}", self.query);
        } else {
            query = self.query.clone();
        }

        return format!("{}{}", path, query);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_parser() {
        let u = Url::new("http://www.my-test.com:8080/whatever?a=1&b=2#test");
        assert!(u.scheme == "http");
        assert!(u.hostname == "www.my-test.com");
        assert!(u.port == "8080");
        assert!(u.path == "/whatever");
        assert!(u.query == "a=1&b=2");
        assert!(u.fragment == "test")
    }
}
