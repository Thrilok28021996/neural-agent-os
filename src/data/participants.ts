import { invoke } from './invoke'

export type CalendarParticipant = { id: string; event_id: string; name: string; email?: string; status: string }
export type ConflictInfo = { event_id: string; title: string; starts_at: string; ends_at: string; overlapping_event_id: string; overlapping_title: string; overlapping_starts_at: string; overlapping_ends_at: string }

export function addParticipant(eventId: string, name: string, email?: string) { return invoke<CalendarParticipant>('add_participant', { eventId, name, email }) }
export function listParticipants(eventId: string) { return invoke<CalendarParticipant[]>('list_participants', { eventId }) }
export function updateParticipantStatus(participantId: string, status: string) { return invoke('update_participant_status', { participantId, status }) }
export function removeParticipant(participantId: string) { return invoke('remove_participant', { participantId }) }
export function detectConflicts(workspaceId: string, startsAt: string, endsAt: string, excludeEventId?: string) { return invoke<ConflictInfo[]>('detect_conflicts', { workspaceId, startsAt, endsAt, excludeEventId }) }
export function checkConflictsForEvent(workspaceId: string, eventId: string) { return invoke<ConflictInfo[]>('check_conflicts_for_event', { workspaceId, eventId }) }

export type InvitationResult = { participant_id: string; sent_to: string; ics_attached: boolean }
export function sendCalendarInvitation(eventId: string, participantId: string) { return invoke<InvitationResult>('send_calendar_invitation', { eventId, participantId }) }
export function processRsvpResponse(participantId: string, response: string) { return invoke('process_rsvp_response', { participantId, response }) }
export function generateIcs(eventId: string) { return invoke<string>('generate_ics', { eventId }) }
