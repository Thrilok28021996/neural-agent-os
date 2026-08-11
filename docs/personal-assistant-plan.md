# Local Personal Assistant: Implementation Plan

## 1. Vision and Product Scope

Build a local-first, single-user desktop personal assistant for macOS, Windows, and Linux. The assistant records and processes meetings, creates accurate multilingual transcripts with speaker diarization, summarizes conversations, manages tasks and reminders, and builds a cited knowledge base from the user's information.

The application will:

- Require no user account for the initial release.
- Store data on a user-selected local device location.
- Work offline for core capture, transcription, search, and assistant workflows.
- Support optional cloud models without requiring a hosted application backend.
- Automatically detect and process meetings, subject to user-configured recording behavior.
- Support workspaces/projects so knowledge can be isolated by context.
- Provide proactive reminders and scheduled autonomous agents.
- Interoperate with Claude Code, Codex, and OpenCode without embedding those tools directly.

The first release is intentionally broad and should be treated as a complete desktop platform rather than a narrow meeting-notes MVP.

## 2. Recommended Technology Architecture

### Desktop application

- **Desktop shell:** Tauri
- **Frontend:** React and TypeScript
- **Native/system layer:** Rust
- **Local background workers:** Rust services where possible, with isolated Python workers for audio/ML libraries when they provide materially better model support
- **Structured storage:** SQLite
- **Text search:** SQLite FTS5
- **Vector search:** Local vector index, preferably SQLite-compatible initially to reduce operational complexity
- **Background jobs:** Durable local job queue with retries and progress tracking
- **Optional hosted component:** Docker-deployed Teams meeting bot
- **Packaging:** Tauri installers for macOS, Windows, and Linux

The application should use stable internal APIs between the desktop UI, native services, model adapters, ingestion pipeline, and automation runtime. This keeps providers and model runtimes replaceable.

## 3. Major Subsystems

1. Desktop shell, permissions, notifications, and system integration
2. Assistant chat and voice interface
3. Model provider abstraction and routing
4. Meeting detection, recording, and import
5. Transcription, diarization, and voice profiles
6. Knowledge ingestion, indexing, retrieval, and citations
7. Built-in calendar and external calendar synchronization
8. Email synchronization and assistant actions
9. Tasks, reminders, and notifications
10. Autonomous-agent scheduler and execution runtime
11. Settings, local storage, backups, import, and deletion
12. MCP, REST, CLI, and context-export integrations for coding agents

## 4. Data and Storage Design

Use SQLite for structured records including:

- Workspaces and projects
- Conversations and messages
- Meetings and participants
- Speaker profiles and diarization corrections
- Recordings and transcript segments
- Summaries, action items, memories, and extracted facts
- Tasks, reminders, and calendar events
- Email metadata, threads, labels, and attachments
- Documents, web pages, and imported sources
- Agent definitions, schedules, runs, and approvals
- Model/provider configuration
- Processing status and audit history

Store large objects separately under the user-selected application data directory:

- Audio and video recordings
- Imported documents and attachments
- Generated exports
- Local model caches
- Search indexes where necessary

Every indexed item must preserve source metadata:

- Source file, URL, email, or meeting
- Meeting timestamp and transcript segment
- Workspace/project
- Creation and modification dates
- Processing status
- Parent/derived relationships

## 5. Workspaces and Knowledge Base

Users can create workspaces such as Personal, Work, Research, and individual projects. Each workspace can define:

- Designated ingestion folders
- Included meetings, documents, email, notes, and web pages
- Assistant context boundaries
- Model preferences
- Autonomous agents
- External coding-agent permissions

The knowledge system will provide:

- Keyword search
- Semantic search
- Hybrid ranking
- Optional reranking
- Workspace and source-type filtering
- Duplicate detection
- Incremental indexing
- Manual re-indexing
- Source citations
- Meeting timestamp citations

The assistant must cite the source behind factual answers whenever information comes from the knowledge base.

Deletion must be explicit and user-controlled. The user can select and delete individual or related items, including:

- Source files
- Meetings
- Recordings
- Transcript segments
- Summaries
- Memories
- Embeddings
- Extracted tasks
- Complete workspaces
- All indexed data

Before deletion, the application should show related derived data and let the user choose which items to remove.

## 6. Meeting Capture

### Automatic detection

Detect meetings using:

- Calendar events
- Meeting URLs
- Supported Teams meeting metadata
- User-configured rules

Recording behavior must be configurable as:

- Automatically record matching meetings
- Ask before recording
- Never record automatically
- Record only selected calendars, workspaces, domains, or participants

The application must display an ongoing recording indicator and configurable recording notices.

### Microsoft Teams

Use a Docker-deployed meeting bot for Teams meetings when bot participation is required. The bot should:

- Join scheduled meetings
- Capture participant audio
- Collect available participant identity metadata
- Transfer encrypted meeting data to the local application
- Report join, recording, upload, and processing status
- Run locally or on a private server
- Fail gracefully when it cannot join or authenticate

The plan must treat Teams bot hosting, permissions, meeting policies, and provider restrictions as explicit deployment risks.

### Local meetings

Support desktop capture of:

- Microphone audio
- System audio
- Separate microphone and system tracks where supported
- Combined tracks when separate capture is unavailable

### Uploaded files

Support drag-and-drop and batch import of audio and video files, including existing meeting recordings.

## 7. Transcription, Diarization, and Voice Profiles

Support English, Hindi, and Telugu for transcription and voice interaction where the selected models support each language.

The processing pipeline should support these deployment modes:

- Fully local transcription and diarization
- Local transcription with cloud summarization
- Cloud transcription with local diarization
- Fully cloud-based processing after explicit user configuration

Diarization should combine:

1. Calendar and invitation metadata
2. Meeting participant metadata
3. User-created voice profiles
4. Profiles learned from corrected transcripts
5. Manual speaker labeling and correction
6. Optional Teams bot identity mapping

Voice profiles and speaker embeddings remain local by default, even when cloud models process transcripts or audio.

The UI should expose confidence scores, speaker corrections, transcript editing, and reprocessing controls.

## 8. Built-in Calendar

Implement a local calendar as the primary calendar interface. It synchronizes with:

- Google Calendar
- Microsoft Outlook Calendar
- Locally created events

Required behavior:

- Create, edit, and delete events
- Recurring events
- Meeting URLs
- Participant invitations
- Time zones
- Meeting preparation workflows
- Automatic recording rules
- Reminders
- Offline changes with later synchronization
- Conflict detection and resolution
- Synchronization status and error reporting

Use preconfigured OAuth integrations for Google and Microsoft where possible, so users do not need to create or maintain a Google Cloud project. Keep provider-specific authentication details isolated from the calendar domain model.

## 9. Email Integration

Use standard IMAP/SMTP as the baseline integration, with first-class support for:

- Gmail
- Outlook/Microsoft 365

Provide optional manual configuration for other providers and self-hosted mail systems.

Required capabilities:

- Local email synchronization
- Thread reconstruction
- Full-text and semantic search
- Email summarization
- Action-item extraction
- Draft generation
- Sending through SMTP or provider APIs
- Labels and folders
- Workspace assignment
- Local attachment indexing
- User approval before sending

Gmail and Outlook OAuth should be preconfigured by the application. The user should not need to maintain a Google Cloud project. Manual IMAP/SMTP setup remains available where OAuth is unavailable or undesirable.

## 10. Model Providers and Settings

Create a unified provider interface with model selection by capability:

- Chat
- Reasoning
- Transcription
- Diarization
- Summarization
- Embeddings
- Reranking
- Speech-to-text
- Text-to-speech
- Vision

### Local runtimes

Support:

- Ollama
- LM Studio
- llama.cpp
- OpenAI-compatible endpoints
- vLLM and similar endpoints

### Cloud providers

Support:

- OpenAI
- Anthropic
- Google
- OpenCode Go plan
- Other OpenAI-compatible providers

Settings must provide:

- Model selection per capability
- Provider fallback
- Cost limits
- Token limits
- Latency preferences
- Offline-only mode
- Per-workspace model settings
- Connection testing
- Provider health status
- Local API-key storage
- Explicit controls for which data may leave the device

The application must clearly identify when raw audio, transcripts, documents, prompts, or embeddings are sent to a cloud provider.

## 11. Assistant and Autonomous Agents

The assistant should support:

- Knowledge-base questions
- Meeting search
- Daily briefings
- Meeting preparation
- Meeting follow-up
- Action-item tracking
- Email triage
- Research workflows
- Document and folder monitoring
- Reminder creation
- Calendar management
- Task management
- User-defined workflows
- Long-running scheduled agents

Agents require:

- Schedules and triggers
- Workspace scope
- Tool permissions
- Approval requirements
- Execution history
- Retry handling
- Pause and resume
- Notifications
- Failure reporting

Require approval by default for:

- Sending email or messages
- Modifying calendar events
- Deleting data
- Executing commands
- Editing files
- External communications

The user may explicitly relax these rules when scheduling an agent.

## 12. Voice and 3D Interface

Provide a futuristic interface with an optional 3D assistant character. The 3D layer should complement, not replace, conventional workspace views.

Required interaction modes:

- Text chat
- Voice input
- Spoken responses
- Live transcript display
- Meeting status visualization
- Agent activity visualization
- Desktop notifications

The interface should remain usable without the 3D character for accessibility, performance, and troubleshooting.

## 13. External Coding-Agent Integration

Do not embed Claude Code, Codex, or OpenCode directly into the application. Instead expose controlled integration surfaces:

- MCP server
- Local REST API
- CLI
- Context export
- Workspace/project boundaries
- Knowledge-base search
- Transcript and meeting search
- Notes and task tools
- Calendar and email tools
- Approval-aware action tools
- Repository and file context access

The user can opt into these integrations. Permissions must define whether an external agent can:

- Read knowledge
- Read transcripts
- Create notes
- Create reminders
- Modify tasks
- Schedule events
- Draft or send email
- Modify project files
- Execute commands

The default integration should be read-only until the user grants additional permissions.

## 14. Privacy and Storage Policy

Initial requirements:

- Local-first storage
- User-selected data directory
- No mandatory application account
- No mandatory hosted backend
- Configurable cloud data sharing
- Configurable recording notices
- Permanent recording retention by default
- User-controlled deletion
- Import and export support

Advanced application encryption, application lock, and enhanced backup protection are not required for the first implementation scope. API keys should nevertheless use the operating system keychain where available.

## 15. Development Phases

### Phase 1: Platform foundation

- Tauri shell
- React interface
- SQLite schema and migrations
- Local storage manager
- Settings system
- Workspace management
- Provider abstraction
- Basic assistant chat

### Phase 2: Knowledge base

- Designated-folder ingestion
- Document parsing
- Notes
- Web-page import
- Conversation indexing
- Hybrid search
- Citations
- Workspace filtering
- Deletion workflows

### Phase 3: Meeting pipeline

- Calendar-based detection
- Local microphone and system capture
- Upload processing
- Offline transcription
- Diarization
- Voice profiles
- Speaker correction
- Summaries
- Action-item extraction

### Phase 4: Calendar and email

- Built-in calendar
- Google Calendar synchronization
- Outlook synchronization
- Gmail integration
- Outlook/Microsoft 365 email integration
- Generic IMAP/SMTP support
- Email indexing
- Email assistant actions

### Phase 5: Automation

- Tasks and reminders
- Desktop notifications
- Daily briefings
- Meeting preparation and follow-up
- Scheduled autonomous agents
- Approval workflows
- Execution history

### Phase 6: Teams bot

- Docker bot service
- Teams meeting joining
- Participant metadata
- Bot-to-local-app transfer
- Recording and processing status
- Failure and fallback behavior

### Phase 7: Voice and 3D experience

- Voice input
- Text-to-speech
- 3D assistant
- Live meeting interface
- Multilingual speech workflows

### Phase 8: External-agent support

- MCP server
- CLI
- REST API
- Context export
- Claude Code, Codex, and OpenCode setup
- Permission and approval controls

### Phase 9: Distribution and release

- macOS installer
- Windows installer
- Linux packages
- Automatic updates
- Import/export
- Diagnostics
- Performance optimization
- User documentation

## 16. Testing Strategy

Test the following areas:

- Offline operation
- Audio capture reliability
- System-audio permissions
- Teams bot joining and recovery
- Transcription in English, Hindi, and Telugu
- Speaker diarization and correction
- Voice-profile matching
- Calendar synchronization conflicts
- Gmail and Outlook synchronization
- Email approval safeguards
- Cloud-provider switching
- Local-model fallback
- Knowledge citations
- Workspace isolation
- Complete deletion of selected derived data
- Agent retries and failures
- External-agent permissions
- Installer and update behavior on all platforms

Use unit tests for domain logic, integration tests for providers and storage, pipeline tests for meeting processing, and end-to-end tests for critical desktop workflows.

## 17. Acceptance Criteria

The first complete release is successful when a user can:

1. Install the application on macOS, Windows, or Linux without creating an application account.
2. Select a local storage location.
3. Create isolated workspaces and configure ingestion folders.
4. Connect Google Calendar, Outlook Calendar, Gmail, and Outlook mail without maintaining a Google Cloud project.
5. Automatically detect configured meetings.
6. Record local meetings with configurable notices and permission behavior.
7. Process Teams meetings through the optional Docker bot.
8. Import audio and video recordings.
9. Generate multilingual transcripts with timestamps and speaker labels.
10. Correct speaker identities and maintain local voice profiles.
11. Search meetings, conversations, documents, web pages, notes, and email with citations.
12. Generate summaries, action items, tasks, reminders, and meeting follow-ups.
13. Select local or cloud models independently for different capabilities.
14. Configure provider fallback, cost limits, and cloud data-sharing rules.
15. Run scheduled autonomous agents with approval controls.
16. Use text and voice interaction with an optional 3D assistant.
17. Delete selected sources and derived data through an understandable deletion workflow.
18. Export or restore user data.
19. Optionally connect Claude Code, Codex, or OpenCode through MCP, CLI, REST, or context export.
20. Keep external coding-agent access restricted to explicitly granted workspaces and tools.

## 18. Principal Risks

- Teams bot APIs, permissions, and tenant policies may limit automatic joining or recording.
- Desktop system-audio capture differs substantially across operating systems.
- Accurate multilingual diarization may require substantial local compute or selected cloud processing.
- Gmail and Outlook authentication policies may change and require provider-specific maintenance.
- Fully autonomous actions create privacy, correctness, and security risks.
- A 3D voice interface can increase memory usage and development complexity.
- Local model support varies by hardware, runtime, quantization, and model capability.
- Large permanent recordings require storage monitoring and clear user controls.

The implementation should build the local data model, provider abstraction, job system, and deletion semantics early because nearly every later feature depends on them.
