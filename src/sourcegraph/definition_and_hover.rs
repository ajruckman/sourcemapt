use crate::sourcegraph::client::SourcegraphClient;
use crate::sourcegraph::error::SourcegraphError;
use graphql_client::GraphQLQuery;
use serde::Deserialize;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/sourcegraph/schema/code_intel_ext.graphql",
    query_path = "src/sourcegraph/query/legacy_definition_and_hover.graphql",
    response_derives = "Debug"
)]
struct LegacyDefinitionAndHover;

type GitObjectID = String;

#[derive(Debug, Deserialize)]
pub struct GetDefinitionResult {
    pub definitions: Vec<DefinitionNode>,
    pub hover: Hover,
}

#[derive(Debug, Deserialize)]
pub struct DefinitionNode {
    pub resource: Resource,
    pub range: Range,
}

#[derive(Debug, Deserialize)]
pub struct Resource {
    pub path: String,
    pub repo: String,
    pub commit_oid: String,
}

#[derive(Debug, Deserialize)]
pub struct Range {
    pub line_start: i64,
    pub char_start: i64,
    pub line_end: i64,
    pub char_end: i64,
}

#[derive(Debug, Deserialize)]
pub struct Hover {
    pub markdown: String,
    pub range: Range,
}

impl SourcegraphClient {
    pub async fn get_definition(
        &self,
        repo: &str,
        rev: &str,
        path: &str,
        line: u32,
        char: u32,
    ) -> Result<Option<GetDefinitionResult>, SourcegraphError> {
        let variables = legacy_definition_and_hover::Variables {
            repository: repo.to_owned(),
            commit: rev.to_owned(),
            path: path.to_owned(),
            line: line as i64,
            character: char as i64,
        };

        let response_body = self
            .post::<LegacyDefinitionAndHover>(variables.into())
            .await
            .map_err(|e| SourcegraphError(format!("failed to get definition: {}", e)))?
            .data
            .ok_or_else(|| SourcegraphError("missing data".to_owned()))?;

        let repository = response_body
            .repository
            .ok_or_else(|| SourcegraphError("missing repository".to_owned()))?;

        let commit = repository
            .commit
            .ok_or_else(|| SourcegraphError("missing commit".to_owned()))?;
        let blob = commit
            .blob
            .ok_or_else(|| SourcegraphError("missing blob".to_owned()))?;
        let lsif = blob
            .lsif
            .ok_or_else(|| SourcegraphError("missing lsif".to_owned()))?;

        if lsif.hover.is_none() {
            return Ok(None);
        }

        let definitions = lsif
            .definitions
            .nodes
            .iter()
            .filter_map(|v| {
                let range = v
                    .range
                    .as_ref()
                    .ok_or_else(|| SourcegraphError("missing range".to_string()))
                    .ok()?;
                Some(DefinitionNode {
                    resource: Resource {
                        path: v.resource.path.clone(),
                        repo: v.resource.repository.name.clone(),
                        commit_oid: v.resource.commit.oid.clone(),
                    },
                    range: Range {
                        line_start: range.start.line,
                        char_start: range.start.character,
                        line_end: range.end.line,
                        char_end: range.end.character,
                    },
                })
            })
            .collect();

        let hover = lsif.hover.as_ref().unwrap();

        let hover = Hover {
            markdown: hover.markdown.text.to_owned(),
            range: Range {
                line_start: hover.range.start.line,
                char_start: hover.range.start.character,
                line_end: hover.range.end.line,
                char_end: hover.range.end.character,
            },
        };

        Ok(Some(GetDefinitionResult { definitions, hover }))
    }
}
