use crate::sourcegraph::client::SourcegraphClient;
use crate::sourcegraph::definition_and_hover;
use graphql_client::reqwest::post_graphql;
use graphql_client::GraphQLQuery;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;

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

impl SourcegraphClient {
    pub async fn get_definition(
        &self,
        repo: &str,
        rev: &str,
        path: &str,
        line: u32,
        char: u32,
    ) -> Result<GetDefinitionResult, Box<dyn Error>> {
        let variables = legacy_definition_and_hover::Variables {
            repository: repo.to_owned(),
            commit: rev.to_owned(),
            path: path.to_owned(),
            line: line as i64,
            character: char as i64,
        };

        let response_body = self
            .post::<LegacyDefinitionAndHover>(variables.into())
            .await?;

        Ok(GetDefinitionResult {})
    }
}
