use chrono::{DateTime, Utc};
use minreq::Method;
use std::collections::BTreeMap;
use sha2::{Sha256, Digest};
use hmac::{Hmac, NewMac};
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Response, Error};

type HmacSha256 = Hmac<Sha256>;

pub enum MessageType {
    Public,
    Private
}

pub struct Client {
    api_key: String,
    api_secret: String
}

impl Client {
    pub fn new<T: Into<String>>(api_key: T, api_secret: T) -> Self {
        Self {
            api_key: api_key.into(),
            api_secret: api_secret.into()
        }
    }

    pub async fn send<T: Into<String>>(
        &self,
        message_type: MessageType,
        endpoint: T,
        mut params: BTreeMap<String, String>
    ) -> Result<Response, Error> {
        let mut uri  = format!("{}{}", crate::CONFIG.kraken.endpoint, endpoint.into());
        match message_type {
            MessageType::Public => {
                if !params.is_empty() {
                    uri = format!("{}?{}", uri, self.create_param_string(params));
                }

                reqwest::Client::new().get(&uri).send().await
            }
            MessageType::Private => {
                let nonce = self.get_nonce();
                params.insert("nonce".to_string(), format!("{}", nonce));

                let body_string = self.create_param_string(params);
                let sig = self.create_signature(
                    uri,
                    nonce,
                    body_string.to_owned()
                );

                reqwest::Client::new()
                    .post(&uri)
                    .headers(self.get_headers(sig))
                    .body(body_string)
                    .send()
                    .await
            }
        }
    }

    fn create_param_string(&self, mut params: BTreeMap<String, String>) -> String {

        let mut param_string = String::new();
        for (key, value) in params.iter() {
            param_string.push_str(format!("{}={}&", key, value).as_str());
        }

        param_string.pop();

        param_string
    }

    fn get_headers(&self, signature: String) -> HeaderMap {
        let mut headers = HeaderMap::new();

        headers.insert("API-Key", HeaderValue::from_str(crate::CONFIG.kraken.api_key.as_str()).expect("Error creating header value."));
        header.insert("API-Sign", HeaderValue::from_str(&signature.as_str()));

        headers
    }

    fn create_signature(
        &self,
        path: String,
        nonce: i64,
        body: String
    ) -> String {
        let digest = Sha256::digest(format!("{}{}", nonce, body).as_bytes());
        let decoded_private_key = base64::decode(&self.api_secret).expect("Oops, you didnt give a secret for the Kraken client.");
        let mac = HmacSha256::new_varkey(&decoded_private_key).expect("Error creating HMAC instance.");

        let mut cheese = path.into_bytes();
        cheese.append(&mut digest.to_vec()). // cheese is the hmac data.

        mac.update(&cheese);

        base64::encode(mac.finalize().into_bytes())
    }

    // We use the current ts as a nonce so we dont have to count /shrug
    fn get_nonce(&self) -> i64 {
        Utc::now().timestamp_millis()
    }
}