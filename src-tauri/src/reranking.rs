use rusqlite::Connection;
use serde::Serialize;

#[derive(Serialize)]
pub struct RerankedResult {
    pub source_id: String,
    pub title: String,
    pub uri: String,
    pub chunk_id: String,
    pub content: String,
    pub relevance_score: f32,
}

/// Rerank search results using a local cross-encoder via Ollama or a simple TF-IDF boost.
/// When Ollama is available, uses it for relevance scoring. Otherwise falls back to
/// keyword-density based scoring.
pub fn rerank_results(
    connection: &Connection,
    workspace_id: &str,
    query: &str,
    results: &[super::SearchResult],
    use_provider: bool,
) -> Result<Vec<RerankedResult>, String> {
    log::info!("reranking::rerank_results: enter");
    if results.is_empty() {
        return Ok(Vec::new());
    }

    if use_provider {
        rerank_with_provider(connection, workspace_id, query, results)
    } else {
        Ok(rerank_local(query, results))
    }
}

fn rerank_local(query: &str, results: &[super::SearchResult]) -> Vec<RerankedResult> {
    let query_lower = query.to_lowercase();
    let query_terms: Vec<&str> = query_lower
        .split_whitespace()
        .collect();
    let mut ranked: Vec<RerankedResult> = results
        .iter()
        .map(|r| {
            let content_lower = r.content.to_lowercase();
            let mut score = 0.0f32;

            // Exact match bonus
            if content_lower.contains(&query_lower) {
                score += 2.0;
            }

            // Term frequency scoring
            for term in &query_terms {
                score += content_lower.matches(term).count() as f32 * 0.5;
            }

            // Title match bonus
            if r.title.to_lowercase().contains(&query_lower) {
                score += 1.0;
            }

            // Normalize by content length to avoid bias toward long chunks
            let length_factor = (r.content.len() as f32 / 200.0).sqrt().max(1.0);
            score /= length_factor;

            RerankedResult {
                source_id: r.source_id.clone(),
                title: r.title.clone(),
                uri: r.uri.clone(),
                chunk_id: r.chunk_id.clone(),
                content: r.content.clone(),
                relevance_score: score,
            }
        })
        .collect();

    ranked.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap_or(std::cmp::Ordering::Equal));
    ranked
}

fn rerank_with_provider(
    connection: &Connection,
    workspace_id: &str,
    query: &str,
    results: &[super::SearchResult],
) -> Result<Vec<RerankedResult>, String> {
    let mut ranked = Vec::new();

    for result in results.iter().take(30) {
        let prompt = format!(
            "Rate how relevant this document chunk is to the query on a scale of 1-10.\n\
             Query: {query}\n\
             Document: {}\n\
             Respond with only a number between 1 and 10.",
            result.content.chars().take(800).collect::<String>()
        );

        // Score via the workspace's routed chat provider (LM Studio by
        // default); any non-numeric answer falls back to a neutral 5.
        let score: f32 = crate::chat::chat_completion(
            connection,
            workspace_id,
            "chat",
            &crate::default_lmstudio_model(),
            &prompt,
        )
        .ok()
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(5.0);

        ranked.push(RerankedResult {
            source_id: result.source_id.clone(),
            title: result.title.clone(),
            uri: result.uri.clone(),
            chunk_id: result.chunk_id.clone(),
            content: result.content.clone(),
            relevance_score: score,
        });
    }

    ranked.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap_or(std::cmp::Ordering::Equal));
    Ok(ranked)
}
