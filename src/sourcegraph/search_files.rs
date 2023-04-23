use graphql_client::reqwest::post_graphql;
use graphql_client::GraphQLQuery;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;

use crate::sourcegraph::search_files::search_files::SearchFilesSearchResultsResults;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/sourcegraph/sourcegraph.graphql",
    query_path = "src/sourcegraph/search_files.graphql",
    response_derives = "Debug"
)]
struct SearchFiles;

#[derive(Serialize, Deserialize, Debug)]
pub struct SearchFilesResult {
    files: Vec<SearchFilesFileMatch>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SearchFilesFileMatch {
    path: String,
    url: String,
    lines: Vec<SearchFilesFileLine>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SearchFilesFileLine {
    line_number: u32,
    preview: String,
}

pub async fn search_files(
    repo: &str,
    search_terms: &[String],
) -> Result<SearchFilesResult, Box<dyn Error>> {
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
        .build()?;

    let query = format!("repo:^{}$", repo.replace(".", r#"\."#));

    let query = format!(
        "{} {}",
        query,
        search_terms
            .iter()
            .map(|term| format!("({})", term))
            .collect::<Vec<String>>()
            .join(" OR ")
    );

    println!("query: {}", query);

    let variables = search_files::Variables { query };

    let response_body =
        post_graphql::<SearchFiles, _>(&client, "https://sourcegraph.com/.api/graphql", variables)
            .await?;

    let response_data: search_files::ResponseData =
        response_body.data.expect("missing response data");
    let output = map(response_data);
    Ok(output)
}

fn map(response_data: search_files::ResponseData) -> SearchFilesResult {
    let mut files = Vec::new();

    match response_data.search {
        None => {}
        Some(data) => {
            for result in data.results.results {
                match result {
                    SearchFilesSearchResultsResults::FileMatch(file) => {
                        let mut lines = Vec::new();

                        for line in file.line_matches {
                            lines.push(SearchFilesFileLine {
                                line_number: line.line_number as u32,
                                preview: line.preview,
                            });
                        }

                        files.push(SearchFilesFileMatch {
                            path: file.file.path,
                            url: file.file.url,
                            lines,
                        });
                    }
                    SearchFilesSearchResultsResults::CommitSearchResult => {}
                    SearchFilesSearchResultsResults::Repository => {}
                }
            }
        }
    }

    SearchFilesResult { files }
}
