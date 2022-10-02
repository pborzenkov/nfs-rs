use url::Url;

/// A trait to try to convert some type into a `Url`.
pub trait IntoUrl: IntoUrlSealed {}

impl IntoUrl for Url {}
impl IntoUrl for String {}
impl<'a> IntoUrl for &'a str {}
impl<'a> IntoUrl for &'a String {}

pub trait IntoUrlSealed {
    fn into_url(self) -> crate::Result<Url>;
}

impl IntoUrlSealed for Url {
    fn into_url(self) -> crate::Result<Url> {
        fn urle(msg: &str) -> crate::Result<Url> {
            Err(crate::error::url(msg))
        }

        if self.scheme() != "nfs" {
            urle("unsupported scheme")?;
        }
        if self.username() != "" || self.password().is_some() {
            urle("either username or password is present")?;
        }
        if !self.has_host() {
            urle("host is missing")?;
        }
        if self.fragment().is_some() {
            urle("fragment is present")?;
        }

        Ok(self)
    }
}

impl IntoUrlSealed for String {
    fn into_url(self) -> crate::Result<Url> {
        (&*self).into_url()
    }
}

impl<'a> IntoUrlSealed for &'a str {
    fn into_url(self) -> crate::Result<Url> {
        Url::parse(self).map_err(crate::error::url)?.into_url()
    }
}

impl<'a> IntoUrlSealed for &'a String {
    fn into_url(self) -> crate::Result<Url> {
        (&**self).into_url()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_url() {
        let url = "nfs://127.0.0.1:1244/share?opt1=val&opt2=val2"
            .into_url()
            .unwrap();
        assert_eq!(
            url.to_string(),
            "nfs://127.0.0.1:1244/share?opt1=val&opt2=val2"
        );
    }

    #[test]
    fn invalid_scheme() {
        let err = "http://127.0.0.1/share".into_url().unwrap_err();
        assert_eq!(err.to_string(), "URL error: unsupported scheme");
    }

    #[test]
    fn username_password() {
        let err = "nfs://user:pass@127.0.0.1/share".into_url().unwrap_err();
        assert_eq!(
            err.to_string(),
            "URL error: either username or password is present"
        );
    }

    #[test]
    fn host_missing() {
        let err = "nfs:/share".into_url().unwrap_err();
        assert_eq!(err.to_string(), "URL error: host is missing")
    }

    #[test]
    fn fragment() {
        let err = "nfs://127.0.0.1/share#fragment".into_url().unwrap_err();
        assert_eq!(err.to_string(), "URL error: fragment is present")
    }

    #[test]
    fn invalid_url() {
        let _ = "nfs/127.0.1share".into_url().unwrap_err();
    }
}
