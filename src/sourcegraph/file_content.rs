use crate::sourcegraph::client::SourcegraphClient;
use graphql_client::GraphQLQuery;
use std::error::Error;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/sourcegraph/schema/sourcegraph.graphql",
    query_path = "src/sourcegraph/query/legacy_file_content.graphql",
    response_derives = "Debug"
)]
struct LegacyFileContent;

pub struct GetFileContentResult {
    pub(crate) content: String,
}

impl SourcegraphClient {
    pub async fn get_file_content(
        &self,
        repo: &str,
        rev: &str,
        path: &str,
    ) -> Result<GetFileContentResult, Box<dyn Error>> {
        let variables = legacy_file_content::Variables {
            repo: repo.to_owned(),
            rev: rev.to_owned(),
            path: path.to_owned(),
        };

        let response_data = self.post::<LegacyFileContent>(variables.into()).await?;

        Ok(GetFileContentResult {
            content: response_data
                .data
                .expect("missing data")
                .repository
                .expect("missing repository")
                .commit
                .expect("missing commit")
                .file
                .expect("missing file")
                .content,
        })
    }
}
