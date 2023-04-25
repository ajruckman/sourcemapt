use reqwest::Client;

pub struct SourcegraphClient {
    pub(crate) client: Client,
}

impl SourcegraphClient {
    pub fn new() -> Self {
        let sourcegraph_api_token =
            std::env::var("SOURCEGRAPH_API_TOKEN").expect("Missing SOURCEGRAPH_API_TOKEN env var");

        let client = Client::builder()
            .user_agent("graphql-rust/0.10.0")
            .default_headers(
                std::iter::once((
                    reqwest::header::AUTHORIZATION,
                    reqwest::header::HeaderValue::from_str(&format!(
                        "Bearer {}",
                        sourcegraph_api_token
                    ))
                        .unwrap(),
                ))
                    .collect(),
            )
            .build()
            .expect("Failed to build reqwest::Client");

        SourcegraphClient { client }
    }
}
