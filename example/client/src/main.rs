use std::time::SystemTime;
use twurst_example_client::{Data, ExampleClient};

#[tokio::main]
async fn main() {
    let data = Data {
        string: "Some String".to_string(),
        time: SystemTime::now(),
    };
    let client = ExampleClient::new("http://localhost:8080/twirp".to_string(), true);
    let response = client
        .test(data.clone())
        .await
        .expect("failed to call test");
    assert_eq!(response, data);
    println!("{response:#?}");
}
