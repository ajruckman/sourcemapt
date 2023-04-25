use std::error::Error;
use graphql_client::GraphQLQuery;
use graphql_client::reqwest::post_graphql;
use reqwest::Client;
use crate::sourcegraph::definition_and_hover;
use serde::{Deserialize, Serialize};

#[derive(GraphQLQuery)]
#[graphql(
schema_path = "src/sourcegraph/schema/code_intel_ext.graphql",
query_path = "src/sourcegraph/query/legacy_definition_and_hover.graphql",
response_derives = "Debug"
)]
struct LegacyDefinitionAndHover;

#[derive(Serialize, Deserialize, Debug)]
pub struct GetDefinitionResult {}

type GitObjectID = String;

// pub async fn get_definition() -> Result<GetDefinitionResult, Box<dyn Error>> {
//     let sourcegraph_api_token =
//         std::env::var("SOURCEGRAPH_API_TOKEN").expect("Missing SOURCEGRAPH_API_TOKEN env var");
//
//     let client = Client::builder()
//         .user_agent("graphql-rust/0.10.0")
//         .default_headers(
//             std::iter::once((
//                 reqwest::header::AUTHORIZATION,
//                 reqwest::header::HeaderValue::from_str(&format!(
//                     "Bearer {}",
//                     sourcegraph_api_token
//                 ))
//                     .unwrap(),
//             ))
//                 .collect(),
//         )
//         .build()?;
//
//     let variables = legacy_definition_and_hover::Variables {  };
//
//     let response_body =
//         post_graphql::<SearchFiles, _>(&client, "https://sourcegraph.com/.api/graphql", variables)
//             .await?;
//
//     Ok(GetDefinitionResult {})
// }
