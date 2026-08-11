import { invoke } from '@tauri-apps/api/core'

export type SourceDeletionPreview = { source_id: string; source_title: string; source_kind: string; chunks: number; embeddings: number; total_size_estimate: number }
export type MeetingDeletionPreview = { meeting_id: string; meeting_title: string; transcript_segments: number; actions: number; summary_exists: boolean; recording_path?: string; recording_size: number }
export type FullDeletionPreview = { workspace_id: string; workspace_name: string; sources: SourceDeletionPreview[]; meetings: MeetingDeletionPreview[]; notes_count: number; emails_count: number; total_derived_items: number; total_storage_impact_bytes: number }

export function previewSourceDeletion(sourceId: string) { return invoke<SourceDeletionPreview>('preview_source_deletion', { sourceId }) }
export function previewMeetingDeletion(meetingId: string) { return invoke<MeetingDeletionPreview>('preview_meeting_deletion', { meetingId }) }
export function fullDeletionPreview(workspaceId: string) { return invoke<FullDeletionPreview>('full_deletion_preview', { workspaceId }) }
