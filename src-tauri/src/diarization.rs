use rusqlite::{params, Connection};
use serde::Serialize;
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
    /// Which backend produced this result: "audio" or "text-embedding".
    pub backend: String,
}

/// One speaker window produced by the audio-based diarizer CLI.
/// Contract: `diarize --json <audio>` prints `{"segments":[{start,end,speaker}]}`.
#[derive(serde::Deserialize)]
pub struct AudioSpeakerWindow {
    pub start: f64,
    pub end: f64,
    pub speaker: String,
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

/// Extract speaker embeddings for a voice profile from transcript samples.
/// Uses the workspace's embedding route (LM Studio by default).mbeddings to create a vector representation of how a speaker talks.
pub fn build_speaker_embedding(
    connection: &Connection,
    profile_id: &str,
    model: &str,
) -> Result<SpeakerEmbedding, String> {
    let (speaker_label, sample_count, workspace_id): (String, i64, String) = connection
        .query_row(
            "SELECT speaker_label, sample_count, workspace_id FROM voice_profiles WHERE id = ?1",
            params![profile_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
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
    let provider = crate::chat::routed_provider(connection, &workspace_id, "embeddings");
    let embedding = crate::embed_text(&provider, model, &combined)?;

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
                serde_json::to_string(&embedding).map_err(|e| e.to_string())?,
                sample_count,
            ],
        )
        .map_err(|e| e.to_string())?;

    Ok(SpeakerEmbedding {
        profile_id: profile_id.to_string(),
        speaker_label,
        embedding,
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
            backend: "text-embedding".to_string(),
        });
    }

    let embed_provider = crate::chat::routed_provider(connection, workspace_id, "embeddings");

    // Simple speaker turn detection with embedding matching
    let mut diarized = Vec::new();
    let mut speaker_set: HashMap<String, u32> = HashMap::new();
    let mut speaker_counter = 0u32;

    for (seg_id, start, end, text) in &segments {
        let mut speaker_label = "Unknown Speaker".to_string();
        let mut confidence = 0.0f32;
        let mut matched_id = None;

        // Get embedding for this segment (provider-aware: LM Studio or Ollama)
        let seg_embedding = crate::embed_text(&embed_provider, embedding_model, text).ok();

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
                "UPDATE transcript_segments SET speaker = ?1, speaker_confidence = ?2 WHERE id = ?3",
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
        backend: "text-embedding".to_string(),
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
#[allow(dead_code)] // tested public API (embedding unit tests)
pub fn get_segment_embedding(
    text: &str,
    model: &str,
) -> Result<Vec<f32>, String> {
    // Default provider is LM Studio / local OpenAI-compatible runtime.
    crate::embed_text("openai_compatible", model, text)
}


/// Fraction of the transcript segment's duration overlapped by the speaker
/// window, used both for choosing the best window and as the assignment
/// confidence.
fn overlap_fraction(seg_start: f64, seg_end: f64, win_start: f64, win_end: f64) -> f64 {
    let overlap = seg_end.min(win_end) - seg_start.max(win_start);
    if overlap <= 0.0 {
        return 0.0;
    }
    (overlap / (seg_end - seg_start).max(0.001)).clamp(0.0, 1.0)
}

/// Run the optional audio-based diarizer (`scripts/diarize.py`, resolved via
/// `NEURAL_DIARIZE_BIN` or PATH) over the recording and assign speaker labels
/// to transcript segments by time overlap.
///
/// Returns:
/// - `Ok(Some(n))` — audio diarization ran and assigned `n` segments;
/// - `Ok(None)`   — the diarizer is not installed (caller falls back to the
///   text-embedding matcher);
/// - `Err(msg)`   — the diarizer is installed but failed to run/parse.
pub fn diarize_audio(
    connection: &Connection,
    meeting_id: &str,
    _workspace_id: &str,
    recording_path: &str,
) -> Result<Option<usize>, String> {
    let diarize = match crate::resolve_binary("diarize", "NEURAL_DIARIZE_BIN") {
        Ok(bin) => bin,
        Err(_) => {
            log::debug!("diarization: audio diarizer not installed; text fallback will be used");
            return Ok(None);
        }
    };
    log::info!("diarization: running audio diarizer {diarize} on {recording_path}");
    let output = std::process::Command::new(&diarize)
        .arg("--json")
        .arg(recording_path)
        .output()
        .map_err(|e| format!("Failed to launch diarizer ({diarize}): {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Diarizer exited with {:?}: {}",
            output.status.code(),
            stderr.trim()
        ));
    }
    #[derive(serde::Deserialize)]
    struct DiarizerOutput {
        segments: Vec<AudioSpeakerWindow>,
    }
    let parsed: DiarizerOutput = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Diarizer JSON parse failed: {e}"))?;
    let windows = parsed.segments;
    if windows.is_empty() {
        log::warn!("diarization: diarizer produced no speaker windows for {recording_path}");
        return Ok(Some(0));
    }

    let mut stmt = connection
        .prepare(
            "SELECT id, start_seconds, end_seconds FROM transcript_segments WHERE meeting_id = ?1 ORDER BY start_seconds",
        )
        .map_err(|e| e.to_string())?;
    let segments: Vec<(String, f64, f64)> = stmt
        .query_map(params![meeting_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;

    let mut assigned = 0usize;
    for (segment_id, seg_start, seg_end) in &segments {
        let mut best: Option<(f64, &AudioSpeakerWindow)> = None;
        for window in &windows {
            let fraction = overlap_fraction(*seg_start, *seg_end, window.start, window.end);
            if fraction > 0.0 && best.map_or(true, |(best_fraction, _)| fraction > best_fraction) {
                best = Some((fraction, window));
            }
        }
        if let Some((fraction, window)) = best {
            let confidence = fraction as f32;
            connection
                .execute(
                    "UPDATE transcript_segments SET speaker = ?1, speaker_confidence = ?2 WHERE id = ?3",
                    params![window.speaker, confidence, segment_id],
                )
                .map_err(|e| e.to_string())?;
            assigned += 1;
        }
    }
    log::info!("diarization: audio backend assigned {assigned}/{} segments for meeting {meeting_id}", segments.len());
    Ok(Some(assigned))
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

    #[test]
    fn overlap_fraction_handles_partial_full_and_none() {
        assert_eq!(overlap_fraction(0.0, 10.0, 2.0, 8.0), 0.6);
        assert_eq!(overlap_fraction(0.0, 10.0, 0.0, 10.0), 1.0);
        assert_eq!(overlap_fraction(0.0, 10.0, 20.0, 30.0), 0.0);
        assert_eq!(overlap_fraction(5.0, 5.0, 0.0, 10.0), 0.0); // zero-length segment
    }

    #[test]
    fn diarize_audio_maps_windows_to_segments() {
        let _env_guard = crate::tests::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // Fake diarizer binary: emits two speaker windows as JSON.
        let dir = std::env::temp_dir().join(format!("nao-diarize-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("fake_diarize.sh");
        std::fs::write(&script, "#!/bin/sh\ncat <<'EOF'\n{\"segments\":[{\"start\":0.0,\"end\":4.0,\"speaker\":\"SPEAKER_00\"},{\"start\":4.0,\"end\":10.0,\"speaker\":\"SPEAKER_01\"}]}\nEOF\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let conn = crate::tests::test_connection();
        conn.execute(
            "INSERT INTO workspaces (id, name) VALUES ('personal', 'Personal')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO meetings (id, workspace_id, title, status) VALUES ('m1', 'personal', 'T', 'transcribed')",
            [],
        )
        .unwrap();
        for (id, start, end) in [("s1", 0.0, 3.0), ("s2", 5.0, 9.0), ("s3", 12.0, 15.0)] {
            conn.execute(
                "INSERT INTO transcript_segments (id, meeting_id, start_seconds, end_seconds, text) VALUES (?1, 'm1', ?2, ?3, 'x')",
                params![id, start, end],
            )
            .unwrap();
        }

        std::env::set_var("NEURAL_DIARIZE_BIN", script.to_string_lossy().into_owned());
        let result = diarize_audio(&conn, "m1", "personal", "/tmp/recording.wav");
        std::env::remove_var("NEURAL_DIARIZE_BIN");
        // Once unset the backend is "not installed" again -> Ok(None).
        let missing = diarize_audio(&conn, "m1", "personal", "/tmp/recording.wav");
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(matches!(missing, Ok(None)), "missing diarizer -> Ok(None), got {missing:?}");

        let assigned = result.expect("audio diarization ran").expect("Some(assigned)");
        assert_eq!(assigned, 2);
        let (s1_speaker, s1_conf): (String, f32) = conn
            .query_row("SELECT speaker, speaker_confidence FROM transcript_segments WHERE id = 's1'", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!(s1_speaker, "SPEAKER_00");
        assert!((s1_conf - 1.0).abs() < 1e-4, "full overlap -> confidence 1.0, got {s1_conf}");
        let (s2_speaker, s2_conf): (String, f32) = conn
            .query_row("SELECT speaker, speaker_confidence FROM transcript_segments WHERE id = 's2'", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!(s2_speaker, "SPEAKER_01");
        // 5.0-9.0 overlaps window 4.0-10.0 fully -> confidence 1.0
        assert!((s2_conf - 1.0).abs() < 1e-4, "got {s2_conf}");
        // s3 overlaps nothing -> untouched
        let (s3_speaker, s3_conf): (Option<String>, Option<f32>) = conn
            .query_row("SELECT speaker, speaker_confidence FROM transcript_segments WHERE id = 's3'", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert!(s3_speaker.is_none());
        assert!(s3_conf.is_none());
    }

}
