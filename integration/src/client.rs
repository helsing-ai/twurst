use crate::proto::{
    self, Int, IntegrationServiceClient, TestRequest, TestResponse, test_request, test_response,
};
use eyre::{Context, OptionExt, Report, Result, bail};
use prost_types::value::Kind;
use prost_types::{Any, Value};
use std::time::{Duration, SystemTime};
use tower::ServiceBuilder;
use tower_http::auth::{AddAuthorization, AddAuthorizationLayer};
use twurst_client::{Reqwest012Service, TwirpHttpClient};

// Simple type to test nested message definition in .proto files
#[derive(Debug, PartialEq, Clone)]
pub enum Choice {
    X,
    Y,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Data {
    pub string: String,
    pub time: SystemTime,
    pub choice: Choice,
    pub duration: Duration,
    pub any: u64,
    pub option: f64,
    pub value: String,
}

pub struct IntegrationClient {
    client: IntegrationServiceClient<AddAuthorization<Reqwest012Service>>,
}

impl IntegrationClient {
    pub fn new(base_url: impl Into<String>, json: bool) -> Self {
        let mut client = TwirpHttpClient::new_with_base(
            ServiceBuilder::new()
                .layer(AddAuthorizationLayer::bearer("password"))
                .service(Reqwest012Service::new()),
            base_url,
        );
        if json {
            client.use_json();
        }
        Self {
            client: IntegrationServiceClient::new(client),
        }
    }

    pub async fn test(&self, data: Data) -> Result<Data> {
        self.client.test(&data.try_into()?).await?.try_into()
    }
}

impl TryFrom<TestResponse> for Data {
    type Error = Report;

    fn try_from(response: TestResponse) -> Result<Self> {
        Ok(Self {
            string: response.string,
            time: response.time.ok_or_eyre("no time")?.try_into()?,
            choice: response
                .nested
                .ok_or_eyre("missing 'nested' field in response")?
                .try_into()
                .wrap_err("failed to turn proto response into Choice")?,
            duration: response.duration.ok_or_eyre("no duration")?.try_into()?,
            any: response.any.ok_or_eyre("no any")?.to_msg::<Int>()?.value,
            option: match response.option.ok_or_eyre("no option")? {
                test_response::Option::Left(_) => bail!("Unexpected option"),
                test_response::Option::Right(value) => value,
            },
            value: match response
                .value
                .ok_or_eyre("no value")?
                .kind
                .ok_or_eyre("no value kind")?
            {
                Kind::StringValue(value) => value,
                value => bail!("Unexpected value {value:?}"),
            },
        })
    }
}

impl TryFrom<proto::test_nested::TestEnum> for Choice {
    type Error = Report;

    fn try_from(value: proto::test_nested::TestEnum) -> std::result::Result<Self, Self::Error> {
        match value {
            proto::test_nested::TestEnum::Unknown => bail!("Unknown TestEnum"),
            proto::test_nested::TestEnum::X => Ok(Choice::X),
            proto::test_nested::TestEnum::Y => Ok(Choice::Y),
        }
    }
}

impl TryFrom<proto::test_nested::NestedMessage> for Choice {
    type Error = Report;

    fn try_from(
        value: proto::test_nested::NestedMessage,
    ) -> std::result::Result<Self, Self::Error> {
        let kind = proto::test_nested::TestEnum::try_from(value.r#enum)
            .wrap_err_with(|| format!("Unknown TestEnum value: {}", value.r#enum))?;

        kind.try_into()
    }
}

impl TryFrom<proto::TestNested> for Choice {
    type Error = Report;

    fn try_from(value: proto::TestNested) -> std::result::Result<Self, Self::Error> {
        value.field0.ok_or_eyre("missing field0")?.try_into()
    }
}

impl TryFrom<Data> for TestRequest {
    type Error = Report;

    fn try_from(data: Data) -> Result<Self> {
        Ok(Self {
            string: data.string,
            time: Some(data.time.into()),
            nested: Some(data.choice.into()),
            duration: Some(data.duration.try_into()?),
            any: Some(Any::from_msg(&Int { value: data.any })?),
            value: Some(Value::from(data.value)),
            option: Some(test_request::Option::Right(data.option)),
        })
    }
}

impl From<Choice> for proto::test_nested::TestEnum {
    fn from(value: Choice) -> Self {
        match value {
            Choice::X => Self::X,
            Choice::Y => Self::Y,
        }
    }
}

impl From<Choice> for proto::test_nested::NestedMessage {
    fn from(value: Choice) -> Self {
        let kind: proto::test_nested::TestEnum = value.into();
        Self {
            r#enum: kind.into(),
        }
    }
}

impl From<Choice> for proto::TestNested {
    fn from(value: Choice) -> Self {
        Self {
            field0: Some(value.into()),
        }
    }
}
