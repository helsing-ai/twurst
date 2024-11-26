use crate::proto::{ExampleServiceClient, TestRequest};
use eyre::{OptionExt, Result};
use std::time::SystemTime;
use twurst_client::{Reqwest012Service, TwirpHttpClient};

mod proto {
    include!(concat!(env!("OUT_DIR"), "/example.rs"));
}

#[derive(Debug, PartialEq, Clone)]
pub struct Data {
    pub string: String,
    pub time: SystemTime,
}

pub struct ExampleClient {
    client: ExampleServiceClient<Reqwest012Service>,
}

impl ExampleClient {
    pub fn new(base_url: String, json: bool) -> Self {
        let mut client = TwirpHttpClient::new_using_reqwest_012(base_url);
        if json {
            client.use_json();
        }
        Self {
            client: ExampleServiceClient::new(client),
        }
    }

    pub async fn test(&self, data: Data) -> Result<Data> {
        let response = self
            .client
            .test(&TestRequest {
                string: data.string,
                time: Some(data.time.into()),
            })
            .await?;
        Ok(Data {
            string: response.string,
            time: response.time.ok_or_eyre("no time")?.try_into()?,
        })
    }
}
