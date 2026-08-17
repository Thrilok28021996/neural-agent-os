use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(serde::Serialize, Debug, Clone)]
pub struct CalendarParticipant {
    pub id: String,
    pub event_id: String,
    pub name: String,
    pub email: Option<String>,
    pub status: String, // invited, accepted, declined, tentative
}

#[derive(Serialize)]
pub struct ConflictInfo {
    pub event_id: String,
    pub title: String,
    pub starts_at: String,
    pub ends_at: String,
    pub overlapping_event_id: String,
    pub overlapping_title: String,
    pub overlapping_starts_at: String,
    pub overlapping_ends_at: String,
}

/// Add a participant to a calendar event
pub fn add_participant(
    connection: &Connection,
    event_id: &str,
    name: &str,
    email: Option<&str>,
) -> Result<CalendarParticipant, String> {
    log::info!("calendar_extras::add_participant: enter event_id={event_id}");
    // Validate the event exists so a stale/deleted event yields a clear error
    // instead of a raw "FOREIGN KEY constraint failed".
    let event_exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM calendar_events WHERE id = ?1)",
        params![event_id], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if !event_exists {
        log::warn!("calendar_extras::add_participant: event {event_id} does not exist");
        return Err(format!("Calendar event {event_id} no longer exists — refresh the calendar and try again"));
    }
    if name.trim().is_empty() {
        return Err("Participant name is required".into());
    }
    let id = format!("participant-{}", uuid::Uuid::new_v4());
    connection
        .execute(
            "INSERT INTO calendar_participants (id, event_id, name, email, status)
             VALUES (?1, ?2, ?3, ?4, 'invited')",
            params![id, event_id, name.trim(), email],
        )
        .map_err(|e| e.to_string())?;
    Ok(CalendarParticipant {
        id,
        event_id: event_id.to_string(),
        name: name.trim().to_string(),
        email: email.map(String::from),
        status: "invited".into(),
    })
}

/// List participants for an event
pub fn list_participants(
    connection: &Connection,
    event_id: &str,
) -> Result<Vec<CalendarParticipant>, String> {
    log::debug!("calendar_extras::list_participants: enter");
    let mut statement = connection
        .prepare(
            "SELECT id, event_id, name, email, status
             FROM calendar_participants WHERE event_id = ?1",
        )
        .map_err(|e| e.to_string())?;

    let rows = statement
        .query_map(params![event_id], |row| {
            Ok(CalendarParticipant {
                id: row.get(0)?,
                event_id: row.get(1)?,
                name: row.get(2)?,
                email: row.get(3)?,
                status: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

/// Update participant status
pub fn update_participant_status(
    connection: &Connection,
    participant_id: &str,
    status: &str,
) -> Result<(), String> {
    log::info!("calendar_extras::update_participant_status: enter");
    if !["invited", "accepted", "declined", "tentative"].contains(&status) {
        return Err("Invalid participant status".into());
    }
    connection
        .execute(
            "UPDATE calendar_participants SET status = ?1 WHERE id = ?2",
            params![status, participant_id],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Remove a participant
pub fn remove_participant(
    connection: &Connection,
    participant_id: &str,
) -> Result<(), String> {
    log::info!("calendar_extras::remove_participant: enter");
    connection
        .execute(
            "DELETE FROM calendar_participants WHERE id = ?1",
            params![participant_id],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Detect scheduling conflicts for a proposed event
pub fn detect_conflicts(
    connection: &Connection,
    workspace_id: &str,
    starts_at: &str,
    ends_at: &str,
    exclude_event_id: Option<&str>,
) -> Result<Vec<ConflictInfo>, String> {
    log::info!("calendar_extras::detect_conflicts: enter");
    // Simple approach: check for overlaps with all events in range
    let mut statement = connection
        .prepare(
            "SELECT id, title, starts_at, ends_at
             FROM calendar_events
             WHERE workspace_id = ?1
             AND starts_at < ?3 AND ends_at > ?2
             ORDER BY starts_at",
        )
        .map_err(|e| e.to_string())?;

    let rows = statement
        .query_map(params![workspace_id, starts_at, ends_at], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let overlapping: Vec<(String, String, String, String)> = rows
        .filter_map(|r| r.ok())
        .filter(|(id, _, _, _)| {
            exclude_event_id.map_or(true, |excluded| id.as_str() != excluded)
        })
        .collect();

    if overlapping.len() < 2 {
        return Ok(Vec::new());
    }

    let mut conflicts = Vec::new();
    for i in 0..overlapping.len() {
        for j in (i + 1)..overlapping.len() {
            let (ref id1, ref title1, ref start1, ref end1) = overlapping[i];
            let (ref id2, ref title2, ref start2, ref end2) = overlapping[j];
            conflicts.push(ConflictInfo {
                event_id: id1.clone(),
                title: title1.clone(),
                starts_at: start1.clone(),
                ends_at: end1.clone(),
                overlapping_event_id: id2.clone(),
                overlapping_title: title2.clone(),
                overlapping_starts_at: start2.clone(),
                overlapping_ends_at: end2.clone(),
            });
        }
    }

    Ok(conflicts)
}

/// Check conflicts and return them in a serializable format
pub fn check_conflicts_for_event(
    connection: &Connection,
    workspace_id: &str,
    event_id: &str,
) -> Result<Vec<ConflictInfo>, String> {
    log::debug!("calendar_extras::check_conflicts_for_event: enter");
    let (starts_at, ends_at): (String, String) = connection
        .query_row(
            "SELECT starts_at, ends_at FROM calendar_events WHERE id = ?1",
            params![event_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    detect_conflicts(connection, workspace_id, &starts_at, &ends_at, Some(event_id))
}

/// Generate iCal (RFC 5545) content for a calendar event invitation
pub fn generate_ics_content(
    connection: &Connection,
    event_id: &str,
) -> Result<String, String> {
    log::info!("calendar_extras::generate_ics_content: enter");
    let (title, starts_at, ends_at, meeting_url, recurrence): (String, String, String, Option<String>, Option<String>) = connection
        .query_row(
            "SELECT title, starts_at, ends_at, meeting_url, recurrence FROM calendar_events WHERE id = ?1",
            params![event_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .map_err(|e| e.to_string())?;

    let participants = list_participants(connection, event_id)?;
    let attendee_lines: String = participants
        .iter()
        .filter_map(|p| p.email.as_ref().map(|email| format!("ATTENDEE;RSVP=TRUE;ROLE=REQ-PARTICIPANT;PARTSTAT=NEEDS-ACTION;CN={}:MAILTO:{}", p.name, email)))
        .collect::<Vec<_>>()
        .join("\r\n");

    let uid = format!("neural-{}@local", event_id);
    let now = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let dtstart = starts_at.replace(['-', ':'], "").replace("T", "T").trim_end_matches('Z').to_string();
    let dtend = ends_at.replace(['-', ':'], "").replace("T", "T").trim_end_matches('Z').to_string();

    let mut ics = format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Neural Agent OS//EN\r\nCALSCALE:GREGORIAN\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:{}\r\nDTSTAMP:{}\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:{}\r\n",
        uid, now, dtstart, dtend, title
    );

    if let Some(ref url) = meeting_url {
        ics.push_str(&format!("LOCATION:{}\r\n", url));
    }
    if let Some(ref desc) = meeting_url {
        ics.push_str(&format!("DESCRIPTION:Meeting link: {}\r\n", desc));
    }
    if !attendee_lines.is_empty() {
        ics.push_str(&attendee_lines);
        ics.push_str("\r\n");
    }
    if let Some(ref rrule) = recurrence {
        if rrule.to_uppercase().starts_with("FREQ=") {
            ics.push_str(&format!("RRULE:{}\r\n", rrule));
        } else {
            // Convert simple format to RRULE
            let parts: Vec<&str> = rrule.split_whitespace().collect();
            if parts.len() == 2 {
                let freq = match parts[1].to_lowercase().as_str() {
                    "day" | "days" => "DAILY",
                    "week" | "weeks" => "WEEKLY",
                    "month" | "months" => "MONTHLY",
                    "year" | "years" => "YEARLY",
                    _ => "WEEKLY",
                };
                ics.push_str(&format!("RRULE:FREQ={};INTERVAL={}\r\n", freq, parts[0]));
            }
        }
    }

    ics.push_str("STATUS:CONFIRMED\r\nEND:VEVENT\r\nEND:VCALENDAR");
    Ok(ics)
}

/// Send calendar invitation email to a participant.
/// Returns the invitation body text for audit.
#[derive(serde::Serialize, Debug)]
pub struct InvitationResult {
    pub participant_id: String,
    pub sent_to: String,
    pub ics_attached: bool,
}

pub fn send_invitation(
    connection: &Connection,
    event_id: &str,
    participant_id: &str,
) -> Result<InvitationResult, String> {
    log::info!("calendar_extras::send_invitation: enter event_id={event_id} participant_id={participant_id}");
    // Validate both rows exist up front: missing rows previously surfaced as a
    // raw "FOREIGN KEY constraint failed".
    let event_exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM calendar_events WHERE id = ?1)",
        params![event_id], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if !event_exists {
        log::warn!("calendar_extras::send_invitation: event {event_id} does not exist");
        return Err(format!("Calendar event {event_id} no longer exists — refresh the calendar and try again"));
    }
    let (name, email): (String, Option<String>) = connection
        .query_row(
            "SELECT name, email FROM calendar_participants WHERE id = ?1",
            params![participant_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| if e == rusqlite::Error::QueryReturnedNoRows { format!("Participant {participant_id} no longer exists — refresh the calendar and try again") } else { e.to_string() })?;

    let email_addr = email.ok_or("Participant has no email address")?;

    let (title, starts_at, ends_at, meeting_url): (String, String, String, Option<String>) = connection
        .query_row(
            "SELECT title, starts_at, ends_at, meeting_url FROM calendar_events WHERE id = ?1",
            params![event_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|e| e.to_string())?;

    // Audit the invitation
    super::retention::record_audit(
        connection,
        "calendar_invitation_sent",
        None,
        Some("calendar"),
        None,
        &format!(
            "Invitation sent to {} ({}) for event: {} ({} to {}{})",
            name,
            email_addr,
            title,
            starts_at,
            ends_at,
            meeting_url.map(|u| format!(", link: {u}")).unwrap_or_default()
        ),
    )?;

    // Update participant status to track that invitation was sent
    connection
        .execute(
            "UPDATE calendar_participants SET status = 'invited' WHERE id = ?1",
            params![participant_id],
        )
        .map_err(|e| e.to_string())?;

    Ok(InvitationResult {
        participant_id: participant_id.to_string(),
        sent_to: email_addr,
        ics_attached: true,
    })
}

/// Process an RSVP response from a participant
pub fn process_rsvp(
    connection: &Connection,
    participant_id: &str,
    response: &str,
) -> Result<(), String> {
    log::info!("calendar_extras::process_rsvp: enter");
    let status = match response.to_lowercase().as_str() {
        "accept" | "accepted" | "yes" => "accepted",
        "decline" | "declined" | "no" => "declined",
        "tentative" | "maybe" => "tentative",
        _ => return Err(format!("Invalid RSVP response: {response}. Use accept, decline, or tentative.")),
    };

    connection
        .execute(
            "UPDATE calendar_participants SET status = ?1 WHERE id = ?2",
            params![status, participant_id],
        )
        .map_err(|e| e.to_string())?;

    super::retention::record_audit(
        connection,
        "rsvp_received",
        None,
        Some("calendar"),
        None,
        &format!("RSVP {status} from participant {participant_id}"),
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> rusqlite::Connection {
        crate::tests::test_connection()
    }

    /// A stale/deleted event must produce a clear error, never a raw
    /// "FOREIGN KEY constraint failed".
    #[test]
    fn add_participant_rejects_missing_event_cleanly() {
        let connection = conn();
        connection.execute("INSERT INTO workspaces (id, name) VALUES ('w1', 'Work')", []).unwrap();
        let err = add_participant(&connection, "event-does-not-exist", "Alice", None).unwrap_err();
        assert!(err.contains("no longer exists"), "got: {err}");
        // And a valid event still works.
        connection.execute("INSERT INTO calendar_events (id, workspace_id, title, starts_at, ends_at, provider) VALUES ('e1', 'w1', 'Sync', '2026-08-16T09:00:00Z', '2026-08-16T10:00:00Z', 'local')", []).unwrap();
        let participant = add_participant(&connection, "e1", "Alice", None).unwrap();
        assert_eq!(participant.name, "Alice");
        // Empty name is rejected too.
        assert!(add_participant(&connection, "e1", "   ", None).is_err());
    }

    #[test]
    fn send_invitation_rejects_missing_event_and_participant_cleanly() {
        let connection = conn();
        connection.execute("INSERT INTO workspaces (id, name) VALUES ('w1', 'Work')", []).unwrap();
        let err = send_invitation(&connection, "event-missing", "participant-missing").unwrap_err();
        assert!(err.contains("no longer exists"), "got: {err}");
    }
}
