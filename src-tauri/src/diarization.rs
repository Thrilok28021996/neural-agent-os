use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Clone)]
pub struct SpeakerEmbedding {
    pub profile_id: String,
    pub speaker_label: String,
    pub embedding: Vec<f32>,
    pub sample_count: i64,
}

#[derive(Serialize)]
pub struct DiarizationResult {
    pub segments: Vec<DiarizedSegment>,
    pub speaker_count: usize,
    pub embedding_model: String,
}

#[derive(Serialize, Clone)]
pub struct DiarizedSegment {
    pub segment_id: String,
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub text: String,
    pub speaker_label: String,
    pub confidence: f32,
    pub matched_profile_id: Option<String>,
}

#[derive(Deserialize)]
struct OllamaEmbedResponse {
    embedding: Vec<f32>,
}

/// Extract speaker embeddings for a voice profile from transcript samples.
/// Uses Ollama embeddings to create a vector representation of how a speaker talks.
pub fn build_speaker_embedding(
    connection: &Connection,
    profile_id: &str,
    model: &str,
) -> Result<SpeakerEmbedding, String> {
    let (speaker_label, sample_count): (String, i64) = connection
        .query_row(
            "SELECT speaker_label, sample_count FROM voice_profiles WHERE id = ?1",
            params![profile_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    if sample_count == 0 {
        return Err("No transcript samples for this speaker profile".into());
    }

    // Gather all transcript segments linked to this profile
    let mut seg_stmt = connection
        .prepare(
            "SELECT ts.text FROM transcript_segments ts
             JOIN voice_profiles vp ON vp.speaker_label = ts.speaker
             WHERE vp.id = ?1 AND ts.speaker IS NOT NULL
             ORDER BY ts.start_seconds LIMIT 20",
        )
        .map_err(|e| e.to_string())?;

    let samples: Vec<String> = seg_stmt
        .query_map(params![profile_id], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    if samples.is_empty() {
        return Err("No transcript text found for this speaker profile".into());
    }

    let combined = samples.join(" ");
    let endpoint = std::env::var("NEURAL_OLLAMA_EMBEDDINGS_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:11434/api/embeddings".into());

    let response = reqwest::blocking::Client::new()
        .post(&endpoint)
        .json(&serde_json::json!({ "model": model, "prompt": combined }))
        .send()
        .map_err(|e| format!("Ollama embeddings unavailable: {e}"))?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json::<OllamaEmbedResponse>()
        .map_err(|e| e.to_string())?;

    // Store the embedding
    connection
        .execute(
            "INSERT INTO speaker_embeddings (profile_id, model, vector_json, sample_count)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(profile_id) DO UPDATE SET
             model = excluded.model,
             vector_json = excluded.vector_json,
             sample_count = excluded.sample_count,
             updated_at = CURRENT_TIMESTAMP",
            params![
                profile_id,
                model,
                serde_json::to_string(&response.embedding).map_err(|e| e.to_string())?,
                sample_count,
            ],
        )
        .map_err(|e| e.to_string())?;

    Ok(SpeakerEmbedding {
        profile_id: profile_id.to_string(),
        speaker_label,
        embedding: response.embedding,
        sample_count,
    })
}

/// Load all speaker embeddings for a workspace
pub fn load_speaker_embeddings(
    connection: &Connection,
    workspace_id: &str,
) -> Result<Vec<SpeakerEmbedding>, String> {
    let mut stmt = connection
        .prepare(
            "SELECT se.profile_id, vp.speaker_label, se.vector_json, se.sample_count
             FROM speaker_embeddings se
             JOIN voice_profiles vp ON vp.id = se.profile_id
             WHERE vp.workspace_id = ?1",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![workspace_id], |row| {
            let vector_json: String = row.get(2)?;
            let embedding: Vec<f32> =
                serde_json::from_str(&vector_json).unwrap_or_default();
            Ok(SpeakerEmbedding {
                profile_id: row.get(0)?,
                speaker_label: row.get(1)?,
                embedding,
                sample_count: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

/// Cosine similarity between two vectors
fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
    let (mut dot, mut na, mut nb) = (0.0f32, 0.0f32, 0.0f32);
    for i in 0..a.len().min(b.len()) {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na.sqrt() * nb.sqrt())
    }
}

/// Perform diarization on transcript segments using speaker embeddings.
/// Matches each segment to the closest voice profile, or assigns "Unknown" labels.
pub fn diarize_transcript(
    connection: &Connection,
    meeting_id: &str,
    workspace_id: &str,
    embedding_model: &str,
    similarity_threshold: f32,
) -> Result<DiarizationResult, String> {
    // Load known speaker embeddings
    let profiles = load_speaker_embeddings(connection, workspace_id)?;

    // Participant metadata fusion: names from calendar participants (joined via
    // calendar events in this workspace) are used as candidate speaker labels
    // when a segment's text contains the participant's name.
    let mut participant_names: Vec<String> = Vec::new();
    if let Ok(mut pstmt) = connection.prepare(
        "SELECT DISTINCT cp.name FROM calendar_participants cp
         JOIN calendar_events ce ON ce.id = cp.event_id
         WHERE ce.workspace_id = ?1",
    ) {
        if let Ok(rows) = pstmt.query_map(params![workspace_id], |row| row.get::<_, String>(0)) {
            participant_names = rows.filter_map(|r| r.ok()).collect();
        }
    }

    // Get transcript segments for the meeting
    let mut seg_stmt = connection
        .prepare(
            "SELECT id, start_seconds, end_seconds, text
             FROM transcript_segments
             WHERE meeting_id = ?1
             ORDER BY start_seconds",
        )
        .map_err(|e| e.to_string())?;

    let segments: Vec<(String, f64, f64, String)> = seg_stmt
        .query_map(params![meeting_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    if segments.is_empty() {
        return Ok(DiarizationResult {
            segments: Vec::new(),
            speaker_count: 0,
            embedding_model: embedding_model.to_string(),
        });
    }

    let endpoint = std::env::var("NEURAL_OLLAMA_EMBEDDINGS_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:11434/api/embeddings".into());

    // Simple speaker turn detection with embedding matching
    let mut diarized = Vec::new();
    let mut speaker_set: HashMap<String, u32> = HashMap::new();
    let mut speaker_counter = 0u32;

    for (seg_id, start, end, text) in &segments {
        let mut speaker_label = "Unknown Speaker".to_string();
        let mut confidence = 0.0f32;
        let mut matched_id = None;

        // Get embedding for this segment
        let seg_embedding = reqwest::blocking::Client::new()
            .post(&endpoint)
            .json(&serde_json::json!({
                "model": embedding_model,
                "prompt": text,
            }))
            .send()
            .ok()
            .and_then(|r| r.json::<OllamaEmbedResponse>().ok())
            .map(|r| r.embedding);

        if let Some(ref embed) = seg_embedding {
            // Match against known profiles
            let mut best_sim = similarity_threshold;
            for profile in &profiles {
                let sim = cosine_sim(embed, &profile.embedding);
                if sim > best_sim {
                    best_sim = sim;
                    speaker_label = profile.speaker_label.clone();
                    confidence = sim;
                    matched_id = Some(profile.profile_id.clone());
                }
            }

            // If no profile matched, try participant metadata: if the segment
            // text mentions a known participant's name, label it accordingly.
            if matched_id.is_none() && confidence < similarity_threshold {
                let lower = text.to_lowercase();
                if let Some(name) = participant_names.iter().find(|name| {
                    !name.trim().is_empty() && lower.contains(&name.to_lowercase())
                }) {
                    speaker_label = name.clone();
                    confidence = 0.6;
                } else {
                    speaker_counter += 1;
                    speaker_label = format!("Speaker {}", speaker_counter);
                    confidence = 0.5; // baseline for new detection
                }
            }
        }

        speaker_set.entry(speaker_label.clone()).or_insert(1);

        // Update the segment in database
        connection
            .execute(
                "UPDATE transcript_segments SET speaker = ?1, confidence = ?2 WHERE id = ?3",
                params![speaker_label, confidence, seg_id],
            )
            .map_err(|e| e.to_string())?;

        diarized.push(DiarizedSegment {
            segment_id: seg_id.clone(),
            start_seconds: *start,
            end_seconds: *end,
            text: text.clone(),
            speaker_label,
            confidence,
            matched_profile_id: matched_id,
        });
    }

    // Merge adjacent same-speaker segments for cleaner output
    let merged = merge_adjacent_speakers(diarized);

    Ok(DiarizationResult {
        segments: merged,
        speaker_count: speaker_set.len(),
        embedding_model: embedding_model.to_string(),
    })
}

/// Merge adjacent segments spoken by the same person
fn merge_adjacent_speakers(
    segments: Vec<DiarizedSegment>,
) -> Vec<DiarizedSegment> {
    if segments.is_empty() {
        return segments;
    }

    let mut merged: Vec<DiarizedSegment> = Vec::new();
    let first = segments[0].clone();
    let mut current = DiarizedSegment {
        segment_id: first.segment_id.clone(),
        start_seconds: first.start_seconds,
        end_seconds: first.end_seconds,
        text: first.text.clone(),
        speaker_label: first.speaker_label.clone(),
        confidence: first.confidence,
        matched_profile_id: first.matched_profile_id.clone(),
    };

    for next in segments.iter().skip(1) {
        if next.speaker_label == current.speaker_label {
            current.end_seconds = next.end_seconds;
            current.text = format!("{} {}", current.text, next.text);
            current.confidence = (current.confidence + next.confidence) / 2.0;
            current.segment_id = next.segment_id.clone();
        } else {
            merged.push(current);
            current = DiarizedSegment {
                segment_id: next.segment_id.clone(),
                start_seconds: next.start_seconds,
                end_seconds: next.end_seconds,
                text: next.text.clone(),
                speaker_label: next.speaker_label.clone(),
                confidence: next.confidence,
                matched_profile_id: next.matched_profile_id.clone(),
            };
        }
    }
    merged.push(current);
    merged
}

/// Get embedding for a single text segment
pub fn get_segment_embedding(
    text: &str,
    model: &str,
) -> Result<Vec<f32>, String> {
    let endpoint = std::env::var("NEURAL_OLLAMA_EMBEDDINGS_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:11434/api/embeddings".into());

    let response = reqwest::blocking::Client::new()
        .post(&endpoint)
        .json(&serde_json::json!({ "model": model, "prompt": text }))
        .send()
        .map_err(|e| format!("Ollama embeddings unavailable: {e}"))?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json::<OllamaEmbedResponse>()
        .map_err(|e| e.to_string())?;

    Ok(response.embedding)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(id: &str, start: f64, end: f64, text: &str, speaker: &str, confidence: f32) -> DiarizedSegment {
        DiarizedSegment {
            segment_id: id.into(),
            start_seconds: start,
            end_seconds: end,
            text: text.into(),
            speaker_label: speaker.into(),
            confidence,
            matched_profile_id: None,
        }
    }

    #[test]
    fn cosine_similarity_of_identical_vectors_is_one() {
        assert!((cosine_sim(&[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0]) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn orthogonal_vectors_have_zero_similarity() {
        assert!(cosine_sim(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-6);
    }

    #[test]
    fn zero_vector_has_no_similarity() {
        assert_eq!(cosine_sim(&[0.0, 0.0], &[1.0, 0.0]), 0.0);
    }

    #[test]
    fn merge_adjacent_combines_same_speaker_segments() {
        let input = vec![
            seg("s1", 0.0, 5.0, "Hello", "Alice", 0.9),
            seg("s2", 5.0, 10.0, "world", "Alice", 0.8),
            seg("s3", 10.0, 15.0, "Next speaker", "Bob", 0.7),
        ];
        let merged = merge_adjacent_speakers(input);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].speaker_label, "Alice");
        assert_eq!(merged[0].text, "Hello world");
        assert_eq!(merged[0].end_seconds, 10.0);
        assert_eq!(merged[1].speaker_label, "Bob");
    }

    #[test]
    fn merge_handles_empty_input() {
        assert!(merge_adjacent_speakers(vec![]).is_empty());
    }
}
