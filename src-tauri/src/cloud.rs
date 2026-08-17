use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct CloudTranscriptionResult {
    pub job_id: String,
    pub meeting_id: String,
    pub provider: String,
    pub model: String,
    pub status: String,
    pub segments: usize,
}

#[derive(Deserialize)]
struct OpenAIWhisperResponse {
    text: String,
    segments: Option<Vec<OpenAISegment>>,
}

#[derive(Deserialize)]
struct OpenAISegment {
    start: f64,
    end: f64,
    text: String,
}

/// Run transcription through a cloud provider with cost enforcement and data-sharing audit
pub fn cloud_transcribe(
    connection: &Connection,
    meeting_id: &str,
    recording_path: &str,
    provider: &str,
    model: &str,
    api_key: &str,
    workspace_id: &str,
    language: &str,
) -> Result<CloudTranscriptionResult, String> {
    log::info!("cloud::cloud_transcribe: meeting={meeting_id} provider={provider} model={model} language={language} audio={recording_path} workspace={workspace_id}");
    // Cost limit enforcement before cloud API call
    let limit = super::costs::get_cost_limit(connection, workspace_id, "transcription")?;
    if let Some(limit_cents) = limit {
        if super::costs::check_cost_limit(connection, workspace_id, limit_cents)? {
            return Err(format!(
                "Transcription cost limit of {} cents exceeded. Use a local model or increase the limit.",
                limit_cents
            ));
        }
    }

    // Data-sharing audit: record when raw audio is sent to a cloud provider
    let audio_size = std::fs::metadata(recording_path).map(|m| m.len()).unwrap_or(0);
    super::retention::record_audit(
        connection,
        "cloud_audio_upload",
        Some(provider),
        Some("raw_audio"),
        Some(workspace_id),
        &format!(
            "Sending {} MB audio to {} for transcription via {model}",
            audio_size as f64 / 1_000_000.0, provider
        ),
    )?;
    let job_id = format!("transcription-{}", uuid::Uuid::new_v4());

    connection
        .execute(
            "INSERT INTO transcription_jobs (id, meeting_id, provider, status) VALUES (?1, ?2, ?3, 'processing')",
            params![job_id, meeting_id, format!("{provider}/{model}")],
        )
        .map_err(|e| e.to_string())?;
    connection
        .execute(
            "UPDATE meetings SET status = 'transcription_processing' WHERE id = ?1",
            params![meeting_id],
        )
        .map_err(|e| e.to_string())?;

    let result = match provider {
        "openai" => transcribe_openai(recording_path, model, api_key),
        "anthropic" => Err("Anthropic does not provide a transcription API. Use OpenAI or a local Whisper runtime.".into()),
        "google" => transcribe_google(recording_path, api_key, language),
        _ => Err(format!("Unsupported cloud transcription provider: {provider}")),
    };

    match result {
        Ok(segments) => {
            if !language.trim().is_empty() {
                let _ = connection.execute("UPDATE meetings SET language = ?1 WHERE id = ?2", params![language, meeting_id]);
            }
            connection
                .execute(
                    "DELETE FROM transcript_segments WHERE meeting_id = ?1",
                    params![meeting_id],
                )
                .map_err(|e| e.to_string())?;

            let count = segments.len();
            for (index, segment) in segments.iter().enumerate() {
                connection
                    .execute(
                        "INSERT INTO transcript_segments (id, meeting_id, start_seconds, end_seconds, text) VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![
                            format!("segment-{}-{}", job_id, index),
                            meeting_id,
                            segment.start,
                            segment.end,
                            segment.text.trim()
                        ],
                    )
                    .map_err(|e| e.to_string())?;
            }

            connection
                .execute(
                    "UPDATE transcription_jobs SET status = 'completed', updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
                    params![job_id],
                )
                .map_err(|e| e.to_string())?;
            connection
                .execute(
                    "UPDATE meetings SET status = 'transcribed' WHERE id = ?1",
                    params![meeting_id],
                )
                .map_err(|e| e.to_string())?;

            // Record transcription cost
            let _ = super::costs::record_token_usage(
                connection, workspace_id, "transcription", provider, model,
                audio_size as i64 / 100, // rough estimate: ~1 token per 100 bytes of audio
                count as i64 * 50, // rough estimate: ~50 output tokens per segment
            );

            Ok(CloudTranscriptionResult {
                job_id,
                meeting_id: meeting_id.to_string(),
                provider: provider.to_string(),
                model: model.to_string(),
                status: "completed".into(),
                segments: count,
            })
        }
        Err(error) => {
            connection
                .execute(
                    "UPDATE transcription_jobs SET status = 'failed', error = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
                    params![error, job_id],
                )
                .map_err(|e| e.to_string())?;
            connection
                .execute(
                    "UPDATE meetings SET status = 'transcription_failed' WHERE id = ?1",
                    params![meeting_id],
                )
                .map_err(|e| e.to_string())?;
            Err(error)
        }
    }
}

struct Segment {
    start: f64,
    end: f64,
    text: String,
}

fn transcribe_openai(path: &str, model: &str, api_key: &str) -> Result<Vec<Segment>, String> {
    let audio_data = std::fs::read(path).map_err(|e| format!("Cannot read audio file: {e}"))?;

    let form = reqwest::blocking::multipart::Form::new()
        .text("model", model.to_string())
        .part(
            "file",
            reqwest::blocking::multipart::Part::bytes(audio_data)
                .file_name(format!(
                    "audio.{}",
                    std::path::Path::new(path)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("wav")
                ))
                .mime_str(mime_type(path))
                .map_err(|e| e.to_string())?,
        )
        .text("response_format", "verbose_json")
        .text("timestamp_granularities[]", "segment");

    let response = reqwest::blocking::Client::new()
        .post("https://api.openai.com/v1/audio/transcriptions")
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .map_err(|e| format!("OpenAI API request failed: {e}"))?
        .error_for_status()
        .map_err(|e| {
            let body = e
                .to_string();
            format!("OpenAI API error: {body}")
        })?;

    let whisper: OpenAIWhisperResponse = response
        .json()
        .map_err(|e| format!("Failed to parse OpenAI response: {e}"))?;

    if let Some(segments) = whisper.segments {
        Ok(segments
            .into_iter()
            .map(|s| Segment {
                start: s.start,
                end: s.end,
                text: s.text,
            })
            .collect())
    } else if !whisper.text.is_empty() {
        // Fallback: wrap full text as a single segment
        Ok(vec![Segment {
            start: 0.0,
            end: 0.0,
            text: whisper.text,
        }])
    } else {
        Err("OpenAI returned no transcript segments or text".into())
    }
}

/// Map the app's language codes (en/hi/te) to Google Speech API BCP-47 codes.
fn google_language_code(language: &str) -> String {
    match language.trim() {
        "hi" => "hi-IN".into(),
        "te" => "te-IN".into(),
        "" | "en" => "en-US".into(),
        other => other.into(), // pass through full BCP-47 codes like "kn-IN"
    }
}

fn transcribe_google(path: &str, api_key: &str, language: &str) -> Result<Vec<Segment>, String> {
    let audio_data = std::fs::read(path).map_err(|e| format!("Cannot read audio file: {e}"))?;
    let encoded = base64_encode_bytes(&audio_data);

    let language_code = google_language_code(language);
    // Alternative languages: every supported code except the primary one.
    let alternatives: Vec<&str> = ["hi-IN", "te-IN", "en-US"]
        .into_iter()
        .filter(|code| *code != language_code)
        .collect();
    let body = serde_json::json!({
        "config": {
            "encoding": encoding_for_path(path),
            "sampleRateHertz": 16000,
            "languageCode": language_code,
            "alternativeLanguageCodes": alternatives,
            "enableAutomaticPunctuation": true,
            "enableWordTimestamps": true,
            "model": "latest_long"
        },
        "audio": {
            "content": encoded
        }
    });

    let response = reqwest::blocking::Client::new()
        .post(format!(
            "https://speech.googleapis.com/v1/speech:recognize?key={api_key}"
        ))
        .json(&body)
        .send()
        .map_err(|e| format!("Google Speech API request failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Google Speech API error: {e}"))?;

    let json: serde_json::Value =
        response.json().map_err(|e| format!("Failed to parse Google response: {e}"))?;

    let mut segments = Vec::new();
    if let Some(results) = json["results"].as_array() {
        for result in results {
            if let Some(alternatives) = result["alternatives"].as_array() {
                for alt in alternatives {
                    let text = alt["transcript"].as_str().unwrap_or("").to_string();
                    let mut start = 0.0;
                    let mut end = 0.0;
                    if let Some(words) = alt["words"].as_array() {
                        if let Some(first) = words.first() {
                            if let Some(s) = first["startTime"]
                                .as_str()
                                .and_then(|t| parse_google_time(t))
                            {
                                start = s;
                            }
                        }
                        if let Some(last) = words.last() {
                            if let Some(e) = last["endTime"]
                                .as_str()
                                .and_then(|t| parse_google_time(t))
                            {
                                end = e;
                            }
                        }
                    }
                    segments.push(Segment { start, end, text });
                }
            }
        }
    }

    if segments.is_empty() {
        Err("Google returned no transcript segments".into())
    } else {
        Ok(segments)
    }
}

fn mime_type(path: &str) -> &str {
    match std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "m4a" => "audio/mp4",
        "mp4" => "video/mp4",
        "webm" => "audio/webm",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        _ => "audio/wav",
    }
}

fn encoding_for_path(path: &str) -> &str {
    match std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "flac" => "FLAC",
        "ogg" => "OGG_OPUS",
        "webm" => "WEBM_OPUS",
        _ => "LINEAR16",
    }
}

fn parse_google_time(s: &str) -> Option<f64> {
    let s = s.trim_end_matches('s');
    s.parse::<f64>().ok()
}

fn base64_encode_bytes(input: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let combined = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((combined >> 18) & 63) as usize] as char);
        result.push(CHARS[((combined >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((combined >> 6) & 63) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(combined & 63) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_types_are_mapped_by_extension() {
        assert_eq!(mime_type("recording.mp3"), "audio/mpeg");
        assert_eq!(mime_type("recording.wav"), "audio/wav");
        assert_eq!(mime_type("recording.mp4"), "video/mp4");
        assert_eq!(mime_type("recording.flac"), "audio/flac");
        assert_eq!(mime_type("recording.unknown"), "audio/wav"); // default
    }

    #[test]
    fn google_encodings_are_mapped() {
        assert_eq!(encoding_for_path("a.flac"), "FLAC");
        assert_eq!(encoding_for_path("a.ogg"), "OGG_OPUS");
        assert_eq!(encoding_for_path("a.webm"), "WEBM_OPUS");
        assert_eq!(encoding_for_path("a.wav"), "LINEAR16");
        assert_eq!(encoding_for_path("a.mp3"), "LINEAR16");
    }

    #[test]
    fn google_time_parsing() {
        assert_eq!(parse_google_time("12.5"), Some(12.5));
        assert_eq!(parse_google_time("12.5s"), Some(12.5));
        assert!(parse_google_time("abc").is_none());
    }

    #[test]
    fn base64_encoding_is_standard() {
        assert_eq!(base64_encode_bytes(b"hello"), "aGVsbG8=");
        assert_eq!(base64_encode_bytes(b"a"), "YQ==");
        assert_eq!(base64_encode_bytes(b""), "");
    }
}
