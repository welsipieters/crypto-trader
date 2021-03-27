use crate::exchanges::mandala::{DEFAULT_RECV_WINDOW, MANDALA_API_URL};
use crate::utils::get_timestamp;
use hmac::{Hmac, Mac, NewMac};
use minreq::{Error, Method, Response};
use sha2::Sha256;
use std::collections::BTreeMap;

type HmacSha256 = Hmac<Sha256>;

pub struct Client {
    api_key: String,
    api_secret: String,
}

impl Client {
    pub fn new<T: Into<String>>(api_key: T, api_secret: T) -> Self {
        Self {
            api_key: api_key.into(),
            api_secret: api_secret.into(),
        }
    }

    pub fn request<T: Into<String>>(
        &self,
        method: Method,
        endpoint: T,
        mut params: BTreeMap<String, String>,
        signed: bool,
    ) -> Result<Response, Error> {
        let mut param_string = Self::create_param_string(params);

        if signed {
            param_string = Self::sign_params(param_string);
        }

        minreq::Request::new(
            method,
            format!("{}{}?{}", MANDALA_API_URL, endpoint.into(), param_string),
        )
        .with_header("X-MBX-APIKEY", self.api_key.clone())
        .send()
    }

    pub fn sign_params<T: Into<String>>(params: T) -> String {
        let params_string = params.into();

        let mut mac =
            HmacSha256::new_varkey(crate::CONFIG.mandala.api_secret.as_bytes()).expect("Error creating hmac");
        mac.update(params_string.clone().as_bytes());
        let result = mac.finalize().into_bytes();
        let signature = hex::encode(result.as_slice());

        format!("{}&signature={}", params_string, signature)
    }

    pub fn create_param_string(mut params: BTreeMap<String, String>) -> String {
        params.insert("recvWindow".to_string(), DEFAULT_RECV_WINDOW.to_string());
        params.insert("timestamp".to_string(), get_timestamp().to_string());

        let mut param_string = String::new();
        for (key, value) in params.iter() {
            param_string.push_str(format!("{}={}&", key, value).as_str());
        }

        param_string.pop();

        param_string
    }
}
