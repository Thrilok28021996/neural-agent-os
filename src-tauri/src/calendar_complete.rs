use rusqlite::{params, Connection};
use chrono::{Utc, Duration, NaiveDateTime, Months};
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct ExpandedEvent {
    pub event_id: String,
    pub instance_date: String,  // RFC3339
    pub title: String,
    pub starts_at: String,
    pub ends_at: String,
    pub meeting_url: Option<String>,
    pub is_recurrence: bool,
}

#[derive(Serialize)]
pub struct TimeZoneInfo {
    pub iana_name: String,
    pub utc_offset_hours: f64,
    pub display_name: String,
}

/// Expand a recurring calendar event into individual instances
pub fn expand_recurring_event(
    connection: &Connection,
    event_id: &str,
    from_date: &str,
    to_date: &str,
) -> Result<Vec<ExpandedEvent>, String> {
    log::info!("calendar_complete::expand_recurring_event: enter");
    let (title, starts_at, ends_at, meeting_url, recurrence): (
        String, String, String, Option<String>, Option<String>,
    ) = connection.query_row(
        "SELECT title, starts_at, ends_at, meeting_url, recurrence FROM calendar_events WHERE id = ?1",
        params![event_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    ).map_err(|e| e.to_string())?;

    let recurrence = match recurrence {
        Some(r) if !r.is_empty() => r,
        _ => {
            // Non-recurring: return single instance
            return Ok(vec![ExpandedEvent {
                event_id: event_id.to_string(),
                instance_date: starts_at.clone(),
                title: title.clone(),
                starts_at: starts_at.clone(),
                ends_at: ends_at.clone(),
                meeting_url: meeting_url.clone(),
                is_recurrence: false,
            }]);
        }
    };

    let start_dt = parse_rfc3339(&starts_at)?;
    let end_dt = parse_rfc3339(&ends_at)?;
    let duration = end_dt - start_dt;
    let from = parse_rfc3339(from_date)?;
    let to = parse_rfc3339(to_date)?;

    let mut instances = Vec::new();
    let mut current = start_dt;

    // Parse recurrence: e.g., "1 week", "1 day", "1 month", "2 weeks"
    let (interval, unit) = parse_recurrence(&recurrence)?;

    for i in 0..365 {
        if current > to { break; }
        if current >= from {
            let instance_end = current + duration;
            instances.push(ExpandedEvent {
                event_id: format!("{}-{}", event_id, i),
                instance_date: current.to_rfc3339(),
                title: title.clone(),
                starts_at: current.to_rfc3339(),
                ends_at: instance_end.to_rfc3339(),
                meeting_url: meeting_url.clone(),
                is_recurrence: i > 0,
            });
        }
        current = match unit.as_str() {
            "day" | "days" => current + Duration::days(interval),
            "week" | "weeks" => current + Duration::weeks(interval),
            "month" | "months" => {
                current.checked_add_months(Months::new(interval as u32))
                    .unwrap_or(current + Duration::days(30 * interval))
            }
            "year" | "years" => {
                current.checked_add_months(Months::new((interval * 12) as u32))
                    .unwrap_or(current + Duration::days(365 * interval))
            }
            _ => break,
        };
    }

    Ok(instances)
}

/// Parse recurrence string. Supports both simple format ("1 week") and iCal RRULE format.
/// RRULE: "FREQ=WEEKLY;INTERVAL=1;BYDAY=MO,WE,FR;COUNT=10" or "FREQ=DAILY;UNTIL=2026-12-31"
fn parse_recurrence(s: &str) -> Result<(i64, String), String> {
    let trimmed = s.trim();

    // Try iCal RRULE format first
    if trimmed.to_uppercase().starts_with("FREQ=") {
        return parse_rrule(trimmed);
    }

    // Simple format: "1 week", "2 days", "1 month", "1 year"
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.len() < 2 {
        return Err(format!("Invalid recurrence: {s}. Use e.g. '1 week' or 'FREQ=WEEKLY'"));
    }
    let interval: i64 = parts[0].parse().map_err(|_| format!("Invalid recurrence interval: {}", parts[0]))?;
    let unit = parts[1].to_lowercase();
    Ok((interval, unit))
}

/// Parse iCal RRULE format: FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE;COUNT=10
fn parse_rrule(rrule: &str) -> Result<(i64, String), String> {
    let mut freq = String::new();
    let mut interval: i64 = 1;

    for part in rrule.split(';') {
        let kv: Vec<&str> = part.splitn(2, '=').collect();
        if kv.len() != 2 { continue; }
        match kv[0].to_uppercase().as_str() {
            "FREQ" => freq = kv[1].to_lowercase(),
            "INTERVAL" => interval = kv[1].parse().unwrap_or(1),
            "COUNT" | "UNTIL" | "BYDAY" | "BYMONTH" | "BYMONTHDAY" => {
                // Acknowledged but not fully expanded in this implementation
            }
            _ => {}
        }
    }

    if freq.is_empty() {
        return Err("RRULE missing FREQ component".into());
    }

    let unit = match freq.as_str() {
        "daily" => "days",
        "weekly" => "weeks",
        "monthly" => "months",
        "yearly" => "years",
        _ => return Err(format!("Unsupported RRULE FREQ: {freq}")),
    };

    Ok((interval, unit.to_string()))
}

fn parse_rfc3339(s: &str) -> Result<chrono::DateTime<Utc>, String> {
    // Handle both formats: with and without timezone
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }
    // Try parsing as naive datetime (assume UTC)
    if let Ok(naive) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Ok(naive.and_utc());
    }
    if let Ok(naive) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M") {
        return Ok(naive.and_utc());
    }
    Err(format!("Cannot parse datetime: {s}"))
}

/// Common timezone information
pub fn common_timezones() -> Vec<TimeZoneInfo> {
    log::info!("calendar_complete::common_timezones: enter");
    vec![
        TimeZoneInfo { iana_name: "America/New_York".into(), utc_offset_hours: -5.0, display_name: "Eastern (UTC-5)".into() },
        TimeZoneInfo { iana_name: "America/Chicago".into(), utc_offset_hours: -6.0, display_name: "Central (UTC-6)".into() },
        TimeZoneInfo { iana_name: "America/Denver".into(), utc_offset_hours: -7.0, display_name: "Mountain (UTC-7)".into() },
        TimeZoneInfo { iana_name: "America/Los_Angeles".into(), utc_offset_hours: -8.0, display_name: "Pacific (UTC-8)".into() },
        TimeZoneInfo { iana_name: "Europe/London".into(), utc_offset_hours: 0.0, display_name: "London (UTC+0)".into() },
        TimeZoneInfo { iana_name: "Europe/Berlin".into(), utc_offset_hours: 1.0, display_name: "Berlin (UTC+1)".into() },
        TimeZoneInfo { iana_name: "Asia/Kolkata".into(), utc_offset_hours: 5.5, display_name: "India (UTC+5:30)".into() },
        TimeZoneInfo { iana_name: "Asia/Tokyo".into(), utc_offset_hours: 9.0, display_name: "Tokyo (UTC+9)".into() },
        TimeZoneInfo { iana_name: "Australia/Sydney".into(), utc_offset_hours: 10.0, display_name: "Sydney (UTC+10)".into() },
    ]
}

/// Convert a datetime string from one timezone to another
pub fn convert_timezone(
    dt_str: &str,
    from_offset_hours: f64,
    to_offset_hours: f64,
) -> Result<String, String> {
    log::info!("calendar_complete::convert_timezone: enter");
    let dt = parse_rfc3339(dt_str)?;
    let adjusted = dt + Duration::seconds(((to_offset_hours - from_offset_hours) * 3600.0) as i64);
    Ok(adjusted.to_rfc3339())
}

/// Auto-detect meeting from calendar event based on configured rules
#[derive(Serialize)]
pub struct RecordingRule {
    pub id: String,
    pub workspace_id: String,
    pub rule_type: String, // calendar, domain, participant, url_pattern
    pub pattern: String,
    pub action: String, // automatic, confirm, disabled
    pub priority: i32,
}

/// Check if an event matches any recording rules
pub fn evaluate_recording_rules(
    connection: &Connection,
    workspace_id: &str,
    event_title: &str,
    meeting_url: Option<&str>,
    participants: &[String],
) -> Result<String, String> {
    log::info!("calendar_complete::evaluate_recording_rules: enter");
    let mut stmt = connection.prepare(
        "SELECT rule_type, pattern, action, priority FROM recording_rules
         WHERE workspace_id = ?1 AND enabled = 1
         ORDER BY priority ASC"
    ).map_err(|e| e.to_string())?;

    let rules: Vec<(String, String, String, i32)> = stmt.query_map(
        params![workspace_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    ).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;

    for (rule_type, pattern, action, _priority) in &rules {
        let matches = match rule_type.as_str() {
            "calendar" => event_title.to_lowercase().contains(&pattern.to_lowercase()),
            "domain" => meeting_url.map(|u| u.contains(pattern.as_str())).unwrap_or(false),
            "participant" => participants.iter().any(|p| p.to_lowercase().contains(&pattern.to_lowercase())),
            "url_pattern" => meeting_url.map(|u| u.contains(pattern.as_str())).unwrap_or(false),
            _ => false,
        };
        if matches {
            return Ok(action.clone());
        }
    }

    // Fall back to workspace default
    let default: String = connection.query_row(
        "SELECT value FROM app_settings WHERE key = 'recordingMode'",
        [], |row| row.get(0),
    ).unwrap_or_else(|_| "confirm".into());
    Ok(default)
}

/// Set a recording rule
pub fn set_recording_rule(
    connection: &Connection,
    workspace_id: &str,
    rule_type: &str,
    pattern: &str,
    action: &str,
    priority: i32,
) -> Result<(), String> {
    log::info!("calendar_complete::set_recording_rule: enter");
    let id = format!("rule-{}", uuid::Uuid::new_v4());
    connection.execute(
        "INSERT INTO recording_rules (id, workspace_id, rule_type, pattern, action, priority, enabled)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)
         ON CONFLICT(id) DO UPDATE SET pattern = excluded.pattern, action = excluded.action, priority = excluded.priority",
        params![id, workspace_id, rule_type, pattern, action, priority],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// List all enabled recording rules for a workspace, ordered by priority.
/// Delete a recording rule.
pub fn delete_recording_rule(connection: &Connection, rule_id: &str) -> Result<(), String> {
    let changed = connection.execute("DELETE FROM recording_rules WHERE id = ?1", params![rule_id])
        .map_err(|e| e.to_string())?;
    if changed == 0 { return Err(format!("Recording rule not found: {rule_id}")); }
    log::info!("calendar_complete::delete_recording_rule: removed {rule_id}");
    Ok(())
}

pub fn list_recording_rules(
    connection: &Connection,
    workspace_id: &str,
) -> Result<Vec<RecordingRule>, String> {
    log::debug!("calendar_complete::list_recording_rules: enter");
    let mut stmt = connection.prepare(
        "SELECT id, workspace_id, rule_type, pattern, action, priority
         FROM recording_rules
         WHERE workspace_id = ?1 AND enabled = 1
         ORDER BY priority ASC",
    ).map_err(|e| e.to_string())?;

    let rows = stmt.query_map(params![workspace_id], |row| {
        Ok(RecordingRule {
            id: row.get(0)?,
            workspace_id: row.get(1)?,
            rule_type: row.get(2)?,
            pattern: row.get(3)?,
            action: row.get(4)?,
            priority: row.get(5)?,
        })
    }).map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::test_connection;

    fn setup() -> Connection {
        let conn = test_connection();
        conn.execute("INSERT INTO workspaces (id, name) VALUES ('w1', 'Work')", []).unwrap();
        conn
    }

    fn insert_event(conn: &Connection, id: &str, title: &str, start: &str, end: &str, recurrence: Option<&str>) {
        conn.execute(
            "INSERT INTO calendar_events (id, workspace_id, title, starts_at, ends_at, meeting_url, recurrence)
             VALUES (?1, 'w1', ?2, ?3, ?4, NULL, ?5)",
            params![id, title, start, end, recurrence],
        ).unwrap();
    }

    #[test]
    fn non_recurring_event_returns_single_instance() {
        let conn = setup();
        insert_event(&conn, "ev-1", "Standup", "2026-08-10T09:00:00Z", "2026-08-10T09:30:00Z", None);
        let instances = expand_recurring_event(&conn, "ev-1", "2026-08-01T00:00:00Z", "2026-08-31T00:00:00Z").unwrap();
        assert_eq!(instances.len(), 1);
        assert!(!instances[0].is_recurrence);
        assert_eq!(instances[0].title, "Standup");
    }

    #[test]
    fn weekly_recurrence_expands_within_window() {
        let conn = setup();
        insert_event(&conn, "ev-1", "Standup", "2026-08-10T09:00:00Z", "2026-08-10T09:30:00Z", Some("1 week"));
        let instances = expand_recurring_event(&conn, "ev-1", "2026-08-01T00:00:00Z", "2026-09-14T00:00:00Z").unwrap();
        assert_eq!(instances.len(), 5); // Aug 10, 17, 24, 31, Sep 7
        assert!(instances[0].is_recurrence == false);
        assert!(instances[1].is_recurrence);
        assert_eq!(instances[1].starts_at, "2026-08-17T09:00:00+00:00");
    }

    #[test]
    fn rrule_weekly_interval_two() {
        let conn = setup();
        insert_event(&conn, "ev-1", "Sync", "2026-08-10T10:00:00Z", "2026-08-10T11:00:00Z", Some("FREQ=WEEKLY;INTERVAL=2"));
        let instances = expand_recurring_event(&conn, "ev-1", "2026-08-01T00:00:00Z", "2026-09-14T00:00:00Z").unwrap();
        assert_eq!(instances.len(), 3); // Aug 10, Aug 24, Sep 7
        assert_eq!(instances[1].starts_at, "2026-08-24T10:00:00+00:00");
    }

    #[test]
    fn invalid_recurrence_is_rejected() {
        let conn = setup();
        insert_event(&conn, "ev-1", "X", "2026-08-10T09:00:00Z", "2026-08-10T09:30:00Z", Some("bogus"));
        assert!(expand_recurring_event(&conn, "ev-1", "2026-08-01T00:00:00Z", "2026-08-31T00:00:00Z").is_err());
    }

    #[test]
    fn convert_timezone_shifts_offset() {
        let converted = convert_timezone("2026-08-10T10:00:00Z", 0.0, 5.5).unwrap();
        assert_eq!(converted, "2026-08-10T15:30:00+00:00");
        let back = convert_timezone(&converted, 5.5, 0.0).unwrap();
        assert_eq!(back, "2026-08-10T10:00:00+00:00");
    }

    #[test]
    fn common_timezones_has_expected_entries() {
        let tzs = common_timezones();
        assert_eq!(tzs.len(), 9);
        assert!(tzs.iter().any(|t| t.iana_name == "Asia/Kolkata" && t.utc_offset_hours == 5.5));
    }

    #[test]
    fn recording_rules_are_evaluated_by_priority() {
        let conn = setup();
        set_recording_rule(&conn, "w1", "calendar", "standup", "automatic", 1).unwrap();
        set_recording_rule(&conn, "w1", "domain", "zoom.us", "confirm", 2).unwrap();

        let action = evaluate_recording_rules(&conn, "w1", "Daily Standup", Some("https://zoom.us/j/123"), &[]).unwrap();
        assert_eq!(action, "automatic"); // calendar rule wins (priority 1)

        let action2 = evaluate_recording_rules(&conn, "w1", "Design Review", Some("https://zoom.us/j/123"), &[]).unwrap();
        assert_eq!(action2, "confirm");

        // No match -> workspace default
        let action3 = evaluate_recording_rules(&conn, "w1", "Design Review", None, &[]).unwrap();
        assert_eq!(action3, "confirm"); // default from app_settings fallback

        let rules = list_recording_rules(&conn, "w1").unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].rule_type, "calendar");
    }

    #[test]
    fn recording_rule_can_be_deleted() {
        let conn = setup();
        set_recording_rule(&conn, "w1", "calendar", "standup", "automatic", 1).unwrap();
        let rules = list_recording_rules(&conn, "w1").unwrap();
        assert_eq!(rules.len(), 1);
        delete_recording_rule(&conn, &rules[0].id).unwrap();
        assert!(list_recording_rules(&conn, "w1").unwrap().is_empty());
        // Deleting a missing rule is a clean error, not a panic.
        assert!(delete_recording_rule(&conn, "rule-nope").is_err());
    }

    #[test]
    fn participant_rule_matches_case_insensitively() {
        let conn = setup();
        set_recording_rule(&conn, "w1", "participant", "alice", "automatic", 1).unwrap();
        let action = evaluate_recording_rules(&conn, "w1", "Review", None, &["Alice Chen".to_string()]).unwrap();
        assert_eq!(action, "automatic");
    }
}
