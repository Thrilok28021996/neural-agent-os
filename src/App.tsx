import { useEffect, useState, type ReactNode } from 'react'
import { loadSelectedWorkspace, saveSelectedWorkspace, workspaceStore, type Workspace } from './data/workspaces'
import { checkProvider, providerStore, type ModelCapability } from './data/providers'
import { loadSettings, saveSettings, type LocalSettings } from './data/settings'
import { loadNativeSetting, loadNativeWorkspaces, saveNativeSetting, setDataDirectory } from './data/native'
import { createWorkspace, deleteWorkspace, previewWorkspaceDeletion, type WorkspaceDeletionPreview } from './data/workspacesAdmin'
import { ingestDirectory } from './data/ingestion'
import { indexSources } from './data/search'
import { searchSources, type SearchResult } from './data/retrieval'
import { embedSources } from './data/embeddings'
import { semanticSearchSources } from './data/semanticSearch'
import { listOpenActions, type OpenAction } from './data/actions'
import { deleteSource, listSources, type Source } from './data/sources'
import { askAssistant } from './data/assistant'
import { createAgent, listAgentRuns, listAgents, runAgentNow, updateAgent, updateAgentStatus, type Agent, type AgentRun } from './data/agents'
import { startLocalRecording, stopLocalRecording, checkRecordingAllowed, type RecordingDecision } from './data/recording'
import { exportWorkspace, importWorkspace } from './data/export'
import { listModelRoutes, setModelRoute, type ModelRoute } from './data/modelRoutes'
import { importMeetingRecording, listMeetings, type Meeting } from './data/meetings'
import { processTranscriptionJob, queueTranscription } from './data/transcription'
import { listTranscriptSegments, updateTranscriptSpeaker, type TranscriptSegment } from './data/transcripts'
import { summarizeMeeting, type MeetingSummary } from './data/summaries'
import { createTask, deleteTask, listTasks, promoteAction, updateTaskStatus, type Task } from './data/tasks'
import { cancelReminder, deleteReminder, listReminders, notifyReminder, scheduleTaskReminder, type Reminder } from './data/reminders'
import { createCalendarEvent, deleteCalendarEvent, listCalendarEvents, updateCalendarEvent, pullRemoteCalendar, type CalendarEvent } from './data/calendar'
import { connectCalendarProvider, listCalendarConnections, type CalendarConnection } from './data/calendarConnections'
import { connectEmailAccount, listEmailAccounts, listEmails, syncEmailAccount, syncEmailOAuth, type Email, type EmailAccount } from './data/email'
import { createNote, deleteNote, listNotes, searchNotes, updateNote, type Note } from './data/notes'
import { importWebpage } from './data/webpage'
import { createVoiceProfile, deleteVoiceProfile, listVoiceProfiles, type VoiceProfile } from './data/voiceProfiles'
import { hybridSearch } from './data/hybridSearch'
import { findDuplicates, type DuplicateGroup } from './data/duplicates'
import { emailTriage, researchBrief, type TriageItem } from './data/triage'
import { exportContext } from './data/contextExport'
import { importMeetingRecordingsBatch } from './data/meetings'
import { reprocessTranscription } from './data/transcription'
import { updateTranscriptSegmentText } from './data/transcripts'
import { createMemory, deleteMemory, listMemories, type Memory } from './data/memories'
import { startVoiceCapture, stopVoiceCapture } from './data/voiceCapture'
import { getTokenLimit, setTokenLimit } from './data/tokenLimits'
import { providerOptions } from './data/providers'
import { deleteProviderSecret, storeProviderSecret } from './data/providerSecrets'
import { scheduleEventReminder } from './data/calendar'
import type { BotRun } from './data/botRuns'
import { ask, message } from '@tauri-apps/plugin-dialog'
import { pickDirectory } from './data/native'
import { conversationSearch } from './data/conversationSearch'
import { sendEmail } from './data/emailSend'
import { languageSpeechCodes, type AssistantLanguage } from './data/language'
import { detectUpcomingMeetings, type DetectedMeeting } from './data/meetingDetection'
import { generateOAuthUrl, syncCalendarEvents } from './data/calendarSync'
import { extractEmailActions, linkTranscriptToProfile } from './data/automation'
import { addParticipant, checkConflictsForEvent, listParticipants, sendCalendarInvitation, type CalendarParticipant, type ConflictInfo } from './data/participants'
import { prepareMeeting, type MeetingPrep } from './data/meetingPrep'
import { cloudTranscribe } from './data/cloudTranscription'
import { getCostSummary, setCostLimit, type CostSummary } from './data/costTracking'
import { getFallbackChain, setFallbackProvider, type FallbackChain } from './data/fallback'
import { addMonitoredFolder, listMonitoredFolders, removeMonitoredFolder, type MonitoredFolder } from './data/monitoring'
import { rerankSearch } from './data/reranking'
import { ThreeDAssistant } from './components/ThreeDAssistant'
import { getStorageStats, getAuditLog, type StorageStats } from './data/diagnostics'
import { getProviderHealth, refreshProviderHealth, type ProviderHealth } from './data/health'
import { listPendingApprovals, approveRequest, denyRequest, type ApprovalRequest } from './data/approvals'
import { rebuildThreads, createLabel, listLabels, labelEmail, generateEmailDraft, summarizeEmailThread, type EmailThread, type EmailLabel } from './data/emailComplete'
import { expandRecurringEvent, evaluateRecordingRules, setRecordingRule, commonTimezones } from './data/calendarComplete'
import { launchMeetingBot } from './data/meetingBot'
import { fullDeletionPreview, previewSourceDeletion, previewMeetingDeletion, type FullDeletionPreview, type SourceDeletionPreview, type MeetingDeletionPreview } from './data/deletion'
import { getSyncStatus, resolveSyncConflict, type SyncStatus } from './data/syncQueue'
import { generateApiKey, listApiKeys, revokeApiKey, deleteTranscriptSegments, deleteMeetingSummary, deleteMeetingActions, deleteMeetingRecording, setWorkspaceRetention } from './data/security'

type View = 'overview' | 'meetings' | 'capture' | 'notes' | 'knowledge' | 'sources' | 'calendar' | 'mail' | 'agents' | 'models' | 'data' | 'settings' | 'three-d' | 'limits' | 'diagnostics' | 'approvals' | 'threads'

const navigation: Array<{ id: View; label: string; icon: string }> = [
  { id: 'overview', label: 'Overview', icon: '◈' },
  { id: 'meetings', label: 'Meetings', icon: '◉' },
  { id: 'capture', label: 'Capture', icon: '●' },
  { id: 'notes', label: 'Notes', icon: '✎' },
  { id: 'knowledge', label: 'Knowledge', icon: '⌘' },
  { id: 'sources', label: 'Sources', icon: '◫' },
  { id: 'calendar', label: 'Calendar', icon: '▦' },
  { id: 'mail', label: 'Mail', icon: '✉' },
  { id: 'agents', label: 'Agents', icon: '✦' },
  { id: 'models', label: 'Models', icon: '◇' },
  { id: 'three-d', label: '3D View', icon: '◈' },
  { id: 'limits', label: 'Costs', icon: '◇' },
  { id: 'diagnostics', label: 'Diagnostics', icon: '◷' },
  { id: 'approvals', label: 'Approvals', icon: '✓' },
  { id: 'threads', label: 'Threads', icon: '✉' },
  { id: 'data', label: 'Data', icon: '⇩' },
]

const capabilityLabels: Record<ModelCapability, string> = {
  chat: 'Conversation',
  transcription: 'Transcription',
  embeddings: 'Knowledge search',
  speech: 'Voice response',
}

export function App() {
  const [view, setView] = useState<View>('overview')
  const [workspace, setWorkspace] = useState<Workspace>(loadSelectedWorkspace)
  const [settings, setSettings] = useState<LocalSettings>(loadSettings)
  const [providerHealth, setProviderHealth] = useState<Record<ModelCapability, boolean>>(() => Object.fromEntries((Object.keys(providerStore) as ModelCapability[]).map((capability) => [capability, providerStore[capability].healthy])) as Record<ModelCapability, boolean>)
  const [showComposer, setShowComposer] = useState(false)

  useEffect(() => { saveSelectedWorkspace(workspace) }, [workspace])
  useEffect(() => { saveSettings(settings) }, [settings])
  useEffect(() => { void loadNativeSetting('settings').then((value) => { if (value) setSettings(JSON.parse(value) as LocalSettings) }).catch(() => undefined) }, [])
  useEffect(() => { void saveNativeSetting('settings', JSON.stringify(settings)).catch(() => undefined) }, [settings])
  useEffect(() => { void loadNativeWorkspaces().then((nativeWorkspaces) => { if (nativeWorkspaces.length > 0) setWorkspace((current) => nativeWorkspaces.find((item) => item.id === current.id) ?? nativeWorkspaces[0]) }).catch(() => undefined) }, [])
  const [newWorkspaceName, setNewWorkspaceName] = useState('')
  const addWorkspace = async () => { const name = newWorkspaceName.trim(); if (!name) return; try { const created = await createWorkspace(name); workspaceStore.push(created); setWorkspace(created); setNewWorkspaceName('') } catch { /* Native workspace creation reports errors through its command boundary. */ } }
  useEffect(() => {
    for (const capability of Object.keys(providerStore) as ModelCapability[]) {
      void checkProvider(providerStore[capability]).then((healthy) => setProviderHealth((current) => ({ ...current, [capability]: healthy })))
    }
  }, [])

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand"><span className="brand-mark">N</span><span>neural<span className="brand-muted">/os</span></span></div>
        <div className="workspace-switcher">
          <span className="eyebrow">Active workspace</span>
          <select value={workspace.id} onChange={(event) => setWorkspace(workspaceStore.find((item) => item.id === event.target.value) ?? workspaceStore[0])}>
            {workspaceStore.map((item) => <option value={item.id} key={item.id}>{item.name}</option>)}
          </select>
          <span className="workspace-path">{workspace.sourceCount} connected sources</span><div style={{ display: 'flex', gap: '4px', marginTop: '8px' }}><input value={newWorkspaceName} onChange={(e) => setNewWorkspaceName(e.target.value)} onKeyDown={(e) => { if (e.key === 'Enter') void addWorkspace() }} placeholder="New workspace name" style={{ flex: 1, minWidth: 0, border: '1px solid var(--line)', borderRadius: '6px', padding: '6px 8px', color: 'var(--soft)', background: 'transparent', font: '10px system-ui' }} /><button className="workspace-add" style={{ marginTop: 0 }} onClick={() => void addWorkspace()}>+ Add</button></div>
        </div>
        <nav className="nav-list" aria-label="Primary navigation">
          {navigation.map((item) => <button className={`nav-item ${view === item.id ? 'active' : ''}`} key={item.id} onClick={() => setView(item.id)}><span className="nav-icon">{item.icon}</span>{item.label}</button>)}
        </nav>
        <div className="sidebar-footer">
          <button className={`nav-item ${view === 'settings' ? 'active' : ''}`} onClick={() => setView('settings')}><span className="nav-icon">⚙</span>Settings</button>
          <div className="profile"><div className="avatar">T</div><div><strong>Local profile</strong><span>Offline-first</span></div><span className="status-dot" /></div>
        </div>
      </aside>
      <main className="main-content">
        <header className="topbar"><div><span className="eyebrow">Sunday, August 9, 2026</span><h1>{view === 'overview' ? 'Good evening, Thrilok' : navigation.find((item) => item.id === view)?.label ?? 'Settings'}</h1></div><div className="topbar-actions"><span className="sync-label"><span className="status-dot" /> Local engine ready</span><button className="icon-button" aria-label="Notifications">♧</button><button className="primary-button" onClick={() => setShowComposer(true)}>+ Ask Neural</button></div></header>
         {view === 'overview' ? <Overview workspace={workspace} onAsk={() => setShowComposer(true)} providerHealth={providerHealth} /> : view === 'settings' ? <Settings settings={settings} onChange={setSettings} /> : view === 'notes' ? <Notes workspace={workspace} /> : view === 'knowledge' ? <Knowledge workspace={workspace} /> : view === 'sources' ? <Sources workspace={workspace} /> : view === 'meetings' ? <Meetings workspace={workspace} /> : view === 'capture' ? <Capture settings={settings} /> : view === 'agents' ? <Agents workspace={workspace} /> : view === 'models' ? <Models workspace={workspace} /> : view === 'three-d' ? <ThreeDView workspace={workspace} /> : view === 'limits' ? <CostsView workspace={workspace} /> : view === 'diagnostics' ? <DiagnosticsView workspace={workspace} /> : view === 'approvals' ? <ApprovalsView workspace={workspace} /> : view === 'threads' ? <ThreadsView workspace={workspace} /> : view === 'data' ? <Data workspace={workspace} /> : view === 'calendar' ? <Calendar workspace={workspace} /> : view === 'mail' ? <Mail workspace={workspace} /> : <SectionView view={view} />}
      </main>
      {showComposer && <AssistantComposer workspace={workspace} onClose={() => setShowComposer(false)} />}
    </div>
  )
}

function Overview({ workspace, onAsk, providerHealth }: { workspace: Workspace; onAsk: () => void; providerHealth: Record<ModelCapability, boolean> }) {
  return <div className="page-grid"><section className="hero-card"><div className="hero-copy"><span className="eyebrow accent">Your context, in one place</span><h2>Make space for<br /><em>better thinking.</em></h2><p>Neural is quietly organizing {workspace.name.toLowerCase()} while you focus on what matters next.</p><button className="text-button" onClick={onAsk}>Start a conversation <span>↗</span></button></div><div className="orbital"><div className="orbital-ring ring-one" /><div className="orbital-ring ring-two" /><div className="orbital-core">N</div><span className="orbit-label label-one">context</span><span className="orbit-label label-two">memory</span><span className="orbit-label label-three">intent</span></div></section><section className="stats-row"><Stat value={String(workspace.sourceCount).padStart(2, '0')} label="Sources indexed" detail="this workspace" /><Stat value="03" label="Upcoming meetings" detail="next 7 days" /><Stat value="08" label="Open actions" detail="2 due today" /></section><section className="content-card activity-card"><div className="section-heading"><div><span className="eyebrow">Recent activity</span><h3>What Neural knows</h3></div><button className="quiet-button">View all ↗</button></div><div className="activity-list"><Activity icon="◉" title="Weekly product sync" meta="Meeting · 42 min transcript" time="Today, 16:20" tone="violet" /><Activity icon="⌁" title="Design brief / mobile assistant" meta="Document · 8 highlights" time="Today, 14:05" tone="amber" /><Activity icon="✦" title="Follow up with the research team" meta="Action item · Extracted from meeting" time="Yesterday" tone="green" /></div></section><section className="content-card routing-card"><div className="section-heading"><div><span className="eyebrow">Model routing</span><h3>Capability status</h3></div><button className="quiet-button">Configure ↗</button></div><div className="routing-list">{(Object.keys(capabilityLabels) as ModelCapability[]).map((capability) => <div className="routing-item" key={capability}><span>{capabilityLabels[capability]}</span><span className="model-name">{providerStore[capability].model}</span><span className={`pill ${providerStore[capability].local ? 'pill-local' : 'pill-cloud'}`}>{providerStore[capability].local ? 'Local' : 'Cloud'}</span><span className={`health-dot ${providerHealth[capability] ? 'healthy' : ''}`} title={providerHealth[capability] ? 'Provider available' : 'Provider unavailable'} /></div>)}</div></section></div>
}

function Notes({ workspace }: { workspace: Workspace }) {
  const [notes, setNotes] = useState<Note[]>([])
  const [title, setTitle] = useState('')
  const [content, setContent] = useState('')
  const [memories, setMemories] = useState<Memory[]>([])
  const [memoryText, setMemoryText] = useState('')
  useEffect(() => { void listMemories(workspace.id).then(setMemories).catch(() => setMemories([])) }, [workspace.id])
  const saveMemory = async () => { if (!memoryText.trim()) return setMessage('Enter a memory to save.'); try { const memory = await createMemory(workspace.id, memoryText.trim()); setMemories((current) => [memory, ...current]); setMemoryText(''); setMessage('Memory saved.') } catch (error) { setMessage(errMsg(error)) } }
  const removeMemory = async (memory: Memory) => { try { await deleteMemory(memory.id); setMemories((current) => current.filter((item) => item.id !== memory.id)); setMessage('Memory deleted.') } catch (error) { setMessage(errMsg(error)) } }
  const [editingNote, setEditingNote] = useState<Note | null>(null)
  const [query, setQuery] = useState('')
  const [message, setMessage] = useState('')
  useEffect(() => { void listNotes(workspace.id).then(setNotes).catch(() => setNotes([])) }, [workspace.id])
  const save = async () => {
    if (!title.trim()) return setMessage('Add a note title.')
    try {
      if (editingNote) { const updated = await updateNote(editingNote.id, title, content); setNotes((current) => current.map((n) => n.id === updated.id ? updated : n)); setEditingNote(null) }
      else { const created = await createNote(workspace.id, title, content); setNotes((current) => [created, ...current]) }
      setTitle(''); setContent(''); setMessage('Note saved locally.')
    } catch (error) { setMessage(errMsg(error)) }
  }
  const startEdit = (note: Note) => { setEditingNote(note); setTitle(note.title); setContent(note.content) }
  const remove = async (note: Note) => { if (!await nativeConfirm(`Delete "${note.title}"?`)) return; try { await deleteNote(note.id); setNotes((current) => current.filter((n) => n.id !== note.id)); setMessage('Note deleted.') } catch (error) { setMessage(errMsg(error)) } }
  const search = async () => { if (!query.trim()) { void listNotes(workspace.id).then(setNotes).catch(() => setNotes([])); return }; try { setNotes(await searchNotes(workspace.id, query)) } catch { setMessage('Search unavailable.') } }
  return <section className="notes-section"><div className="notes-header"><div><span className="eyebrow accent">{workspace.name} workspace</span><h2>Capture your<br /><em>thoughts.</em></h2><p>Notes are full-text searchable and stay inside your workspace. They're available to the assistant when you ask questions.</p></div><div className="notes-glyph">✎</div></div><div className="ingestion-panel"><span className="eyebrow">{editingNote ? 'Edit note' : 'New note'}</span><div className="agent-form"><input value={title} onChange={(event) => setTitle(event.target.value)} placeholder="Note title" /><textarea value={content} onChange={(event) => setContent(event.target.value)} placeholder="Start writing..." /><div style={{ display: 'flex', gap: '9px' }}><button className="primary-button" onClick={() => void save()}>{editingNote ? 'Update' : 'Save note'}</button>{editingNote && <button className="quiet-button" onClick={() => { setEditingNote(null); setTitle(''); setContent('') }}>Cancel</button>}</div></div>{message && <span className="ingestion-message">{message}</span>}</div><div className="search-panel"><span className="eyebrow">Search notes</span><div className="ingestion-form"><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Find in notes..." onKeyDown={(event) => { if (event.key === 'Enter') void search() }} /><button className="primary-button" onClick={() => void search()}>Search</button></div></div><div className="note-list">{notes.length === 0 ? <div className="empty-list">No notes yet. Write your first one above.</div> : notes.map((note) => <article className="note-item" key={note.id}><div><strong>{note.title}</strong><p>{note.content.slice(0, 200)}{note.content.length > 200 ? '...' : ''}</p><small>{new Date(note.updated_at).toLocaleDateString()}</small></div><div className="note-actions"><button className="quiet-button" onClick={() => startEdit(note)}>Edit</button><button className="quiet-button" onClick={() => void remove(note)}>Delete</button></div></article>)}</div><div className="ingestion-panel"><span className="eyebrow">Memories</span><h3>Durable facts about you</h3><p>Short facts that persist across conversations and appear in workspace context exports.</p><div className="agent-form"><textarea value={memoryText} onChange={(event) => setMemoryText(event.target.value)} placeholder="e.g. User prefers concise replies" /><button className="primary-button" onClick={() => void saveMemory()}>Save memory</button></div><div className="note-list">{memories.length === 0 ? <div className="empty-list">No memories saved yet.</div> : memories.map((memory) => <div key={memory.id} style={{ display: 'flex', alignItems: 'center', gap: '8px', padding: '6px 0', fontSize: '12px', borderBottom: '1px solid var(--line)' }}><span style={{ flex: 1 }}>{memory.content}</span><button className="quiet-button" onClick={() => void removeMemory(memory)} title="Delete memory" style={{ fontSize: '10px' }}>✕</button></div>)}</div></div></section>
}

function Stat({ value, label, detail }: { value: string; label: string; detail: string }) { return <div className="stat"><strong>{value}</strong><div><span>{label}</span><small>{detail}</small></div></div> }
function Activity({ icon, title, meta, time, tone }: { icon: string; title: string; meta: string; time: string; tone: string }) { return <div className="activity-item"><span className={`activity-icon ${tone}`}>{icon}</span><div><strong>{title}</strong><span>{meta}</span></div><time>{time}</time></div> }
function SectionView({ view }: { view: View }) { return <section className="empty-section"><span className="eyebrow">Foundation in progress</span><h2>{view === 'settings' ? 'Everything stays in your hands.' : 'This space is ready for your context.'}</h2><p>The local data layer and provider contracts are being established first. This section will connect to the {view} workflow next.</p><button className="text-button">Review the implementation plan <span>↗</span></button></section> }

function Settings({ settings, onChange }: { settings: LocalSettings; onChange: (settings: LocalSettings) => void }) {
  const update = <K extends keyof LocalSettings>(key: K, value: LocalSettings[K]) => onChange({ ...settings, [key]: value })
  const chooseDirectory = async () => { const path = await pickDirectory(); if (!path) return; try { const selected = await setDataDirectory(path); update('dataDirectory', selected); void nativeAlert('Data directory updated. Restart Neural Agent OS to use the migrated database.') } catch (error) { void nativeAlert(errMsg(error)) } }
  return <section className="settings-section"><div className="settings-intro"><span className="eyebrow accent">Local control center</span><h2>Everything stays<br /><em>in your hands.</em></h2><p>These preferences are stored locally. Changing the data directory copies the current database and takes effect after restart.</p></div><div className="settings-panel"><SettingRow title="Recording behavior" description="Choose how meetings should begin recording."><select value={settings.recordingMode} onChange={(event) => update('recordingMode', event.target.value as LocalSettings['recordingMode'])}><option value="automatic">Record automatically</option><option value="confirm">Ask before recording</option><option value="disabled">Never record automatically</option></select></SettingRow><SettingRow title="Recording notices" description="Show a visible indicator and configurable notice while recording."><Toggle checked={settings.recordingNotice} onChange={(value) => update('recordingNotice', value)} /></SettingRow><SettingRow title="Cloud audio processing" description="Allow raw audio to be sent to a configured cloud provider."><Toggle checked={settings.allowCloudAudio} onChange={(value) => update('allowCloudAudio', value)} /></SettingRow><SettingRow title="Cloud text processing" description="Allow transcripts and prompts to use cloud models."><Toggle checked={settings.allowCloudText} onChange={(value) => update('allowCloudText', value)} /></SettingRow><SettingRow title="Data directory" description="SQLite and local application data are stored here."><div><span className="setting-value">{settings.dataDirectory}</span><button className="quiet-button" onClick={() => void chooseDirectory()}>Choose folder</button></div></SettingRow></div></section>
}

function Knowledge({ workspace }: { workspace: Workspace }) {
  const [directory, setDirectory] = useState('')
  const [url, setUrl] = useState('')
  const [message, setMessage] = useState('')
  const [working, setWorking] = useState(false)
  const [query, setQuery] = useState('')
  const [results, setResults] = useState<SearchResult[]>([])
  const scan = async () => {
    if (!directory.trim()) return setMessage('Enter a folder path to scan.')
    setWorking(true)
    setMessage('Scanning local folder...')
    try {
      const result = await ingestDirectory(workspace.id, directory.trim())
      setMessage(`${result.inserted} new source${result.inserted === 1 ? '' : 's'} added from ${result.directory}.`)
    } catch (error) {
      setMessage(errMsg(error))
    } finally {
      setWorking(false)
    }
  }
  const fetchPage = async () => {
    if (!url.trim()) return setMessage('Enter a URL to import.')
    setWorking(true)
    setMessage('Fetching and indexing webpage...')
    try {
      const page = await importWebpage(workspace.id, url.trim())
      setMessage(`Web page "${page.title}" imported and indexed for search.`)
      setUrl('')
    } catch (error) {
      setMessage(errMsg(error))
    } finally {
      setWorking(false)
    }
  }
  const index = async () => { setWorking(true); setMessage('Indexing text sources...'); try { const result = await indexSources(workspace.id); setMessage(`${result.chunks_indexed} searchable chunks created from ${result.sources_indexed} sources.`) } catch (error) { setMessage(errMsg(error)) } finally { setWorking(false) } }
  const embed = async () => { setWorking(true); setMessage('Creating local semantic embeddings...'); try { const result = await embedSources(workspace.id); setMessage(`${result.chunks_embedded} chunks embedded with ${result.model}.`) } catch (error) { setMessage(errMsg(error)) } finally { setWorking(false) } }
  const search = async () => { if (!query.trim()) return setResults([]); setWorking(true); setMessage('Searching this workspace...'); try { const [found, convos] = await Promise.all([searchSources(workspace.id, query), conversationSearch(workspace.id, query)]); setResults([...found, ...convos]); setMessage(`${found.length + convos.length} result${found.length + convos.length === 1 ? '' : 's'} found.`) } catch (error) { setMessage(errMsg(error)) } finally { setWorking(false) } }
  const semanticSearch = async () => { if (!query.trim()) return setResults([]); setWorking(true); setMessage('Ranking local semantic matches...'); try { const found = await semanticSearchSources(workspace.id, query); setResults(found); setMessage(`${found.length} semantic result${found.length === 1 ? '' : 's'} found.`) } catch (error) { setMessage(errMsg(error)) } finally { setWorking(false) } }
  const hybridSearchAll = async () => { if (!query.trim()) return setResults([]); setWorking(true); setMessage('Merging keyword + semantic rankings...'); try { const found = await hybridSearch(workspace.id, query); setResults(found); setMessage(`${found.length} hybrid result${found.length === 1 ? '' : 's'} found (keyword + semantic).`) } catch (error) { setMessage(errMsg(error)) } finally { setWorking(false) } }
  const [brief, setBrief] = useState('')
  const compileBrief = async () => { if (!query.trim()) return setBrief(''); setWorking(true); setMessage('Compiling research brief...'); try { setBrief(await researchBrief(workspace.id, query)); setMessage('Research brief ready.') } catch (error) { setMessage(errMsg(error)) } finally { setWorking(false) } }
  const [duplicates, setDuplicates] = useState<DuplicateGroup[]>([])
  const scanDuplicates = async () => { setWorking(true); setMessage('Comparing source content hashes...'); try { const groups = await findDuplicates(workspace.id); setDuplicates(groups); setMessage(groups.length ? `${groups.length} duplicate group${groups.length === 1 ? '' : 's'} found.` : 'No duplicate sources found.') } catch (error) { setMessage(errMsg(error)) } finally { setWorking(false) } }
  return <section className="knowledge-section"><div className="knowledge-header"><div><span className="eyebrow accent">{workspace.name} workspace</span><h2>Give Neural<br /><em>something to learn.</em></h2><p>Connect local folders and web pages. Source files are recorded for parsing, chunking, embeddings, and cited retrieval.</p></div><div className="knowledge-glyph">⌘</div></div><div className="ingestion-panel"><span className="eyebrow">Designated folder</span><h3>Scan local sources</h3><p>Supported now: PDF, Markdown, text, DOCX, HTML, JSON, and CSV.</p><div className="ingestion-form"><input value={directory} onChange={(event) => setDirectory(event.target.value)} placeholder="/Users/you/Documents/research" onKeyDown={(event) => { if (event.key === 'Enter') void scan() }} /><button className="primary-button" disabled={working} onClick={() => void scan()}>{working ? 'Scanning...' : 'Scan folder'}</button></div></div><div className="ingestion-panel"><span className="eyebrow">Web page</span><h3>Import from URL</h3><p>Enter any public URL to extract its text content and add it to your searchable knowledge base.</p><div className="ingestion-form"><input value={url} onChange={(event) => setUrl(event.target.value)} placeholder="https://example.com/article" onKeyDown={(event) => { if (event.key === 'Enter') void fetchPage() }} /><button className="primary-button" disabled={working} onClick={() => void fetchPage()}>{working ? 'Fetching...' : 'Import page'}</button></div></div><button className="text-button" disabled={working} onClick={() => void index()}>Build searchable index <span>↗</span></button><button className="text-button" disabled={working} onClick={() => void embed()}>Create semantic embeddings <span>↗</span></button>{message && <span className="ingestion-message">{message}</span>}<div className="search-panel"><span className="eyebrow">Cited retrieval</span><h3>Search your context</h3><div className="ingestion-form"><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="What did we decide about the assistant?" onKeyDown={(event) => { if (event.key === 'Enter') void search() }} /><button className="primary-button" disabled={working} onClick={() => void search()}>Keyword</button><button className="quiet-button" disabled={working} onClick={() => void semanticSearch()}>Semantic ↗</button><button className="quiet-button" disabled={working} onClick={() => void hybridSearchAll()}>Hybrid ↗</button><button className="quiet-button" disabled={working} onClick={() => void compileBrief()}>Research ↗</button></div>{brief && <pre style={{ whiteSpace: 'pre-wrap', fontSize: '11px', lineHeight: 1.6, maxHeight: '280px', overflow: 'auto', border: '1px solid var(--line)', borderRadius: '8px', padding: '12px', marginTop: '10px' }}>{brief}</pre>}<button className="text-button" disabled={working} onClick={() => void scanDuplicates()}>Find duplicate sources <span>↗</span></button>{duplicates.length > 0 && <div className="duplicate-list">{duplicates.map((group) => <article className="search-result" key={group.content_hash}><div><strong>{group.source_count} sources with identical content</strong><span>{group.sources.map((s) => `${s.title} (${s.uri ?? 'no uri'})`).join(' · ')}</span></div></article>)}</div>}{results.length > 0 && <div className="search-results">{results.map((result) => <article className="search-result" key={result.chunk_id}><div><strong>{result.title}</strong><span>{result.content}</span></div><code>{result.uri}</code></article>)}</div>}</div></section>
}

function Sources({ workspace }: { workspace: Workspace }) {
  const [sources, setSources] = useState<Source[]>([])
  const [message, setMessage] = useState('')
  useEffect(() => { void listSources(workspace.id).then(setSources).catch(() => setSources([])) }, [workspace.id])
  const remove = async (source: Source) => { if (!await nativeConfirm(`Delete ${source.title} and its derived index data?`)) return; try { await deleteSource(source.id); setSources((current) => current.filter((item) => item.id !== source.id)); setMessage('Source and derived index data deleted.') } catch (error) { setMessage(errMsg(error)) } }
  return <section className="sources-section"><div className="empty-section source-intro"><span className="eyebrow accent">Knowledge control</span><h2>Know what Neural<br /><em>knows.</em></h2><p>Every indexed source belongs to a workspace. Removing one also removes its searchable chunks and local embeddings.</p>{message && <span className="ingestion-message">{message}</span>}</div><div className="source-panel">{sources.length === 0 ? <div className="empty-list">No sources connected to this workspace.</div> : sources.map((source) => <article className="source-item" key={source.id}><div><strong>{source.title}</strong><span>{source.kind} · {source.status}</span><code>{source.uri ?? 'No location recorded'}</code></div><button className="quiet-button" onClick={() => void remove(source)}>Delete</button></article>)}</div></section>
}

function Capture({ settings }: { settings: LocalSettings }) {
  const tsName = () => { const d = new Date(); const pad = (n: number) => String(n).padStart(2, '0'); return `~/Recordings/meeting-${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}_${pad(d.getHours())}-${pad(d.getMinutes())}-${pad(d.getSeconds())}.m4a` }
  const [path, setPath] = useState(tsName)
  const [pathTouched, setPathTouched] = useState(false)
  const [source, setSource] = useState('default')
  const [active, setActive] = useState(false)
  const [message, setMessage] = useState('')
  const [decision, setDecision] = useState<RecordingDecision | null>(null)
  const [elapsed, setElapsed] = useState(0)
  useEffect(() => { if (!active) { setElapsed(0); return }; const timer = window.setInterval(() => setElapsed((s) => s + 1), 1000); return () => window.clearInterval(timer) }, [active])
  const evaluate = async () => { try { setDecision(await checkRecordingAllowed(undefined, undefined, undefined, [])) } catch (error) { setDecision(null); setMessage(errMsg(error)) } }
  const start = async () => {
    try {
      // Timestamp-named output: unless the user customized the path, name the
      // file with the moment recording actually starts.
      const recordingPath = pathTouched ? path.trim() : tsName()
      if (!pathTouched) setPath(recordingPath)
      const decision = await checkRecordingAllowed(undefined, undefined, undefined, [])
      if (!decision.allowed) { setMessage(`Recording blocked: ${decision.reason}`); return }
      if (decision.action === 'confirm' && !await nativeConfirm('Start local recording? Confirm that all participants have been notified.')) return
      await startLocalRecording(recordingPath, source, { confirmed: decision.action !== 'automatic' })
      setActive(true)
      setMessage(settings.recordingNotice ? `Recording to ${recordingPath}. Visible recording notice enabled.` : `Recording to ${recordingPath}. Keep this window open until you stop.`)
    } catch (error) { setMessage(errMsg(error)) }
  }
  const stop = async () => { try { const status = await stopLocalRecording(); setActive(false); setMessage(status.queued ? `Recording saved to ${path}. Transcription queued — it will run automatically.` : `Recording saved to ${path}.`) } catch (error) { setMessage(errMsg(error)) } }
  return <section className="capture-section"><div className={`capture-orb ${active ? 'recording' : ''}`}>{active ? <span style={{ position: 'relative' }}>●<span style={{ position: 'absolute', inset: '-8px', border: '2px solid #ff8496', borderRadius: '50%', animation: 'pulse 1s infinite', opacity: .5 }} /></span> : <span>N</span>}</div><span className="eyebrow accent">Live capture</span><h2>{active ? `Recording · ${Math.floor(elapsed / 60).toString().padStart(2, '0')}:${(elapsed % 60).toString().padStart(2, '0')}` : 'Capture a conversation.'}</h2><p>{active ? 'Conversation is being captured locally. The file will be ready for transcription when you stop.' : 'Record local microphone or system audio into a transcription-ready file. Configure consent notices before using this in shared meetings.'}</p><div className="capture-panel"><label>Output path<input value={path} onChange={(event) => { setPath(event.target.value); setPathTouched(true) }} disabled={active} placeholder="~/Recordings/meeting-YYYY-MM-DD_HH-MM-SS.m4a" /></label><label>Input source<select value={source} onChange={(event) => setSource(event.target.value)} disabled={active}><option value="default">Default microphone</option><option value="system">System audio</option></select></label><div className="capture-actions">{active ? <button className="primary-button" onClick={() => void stop()} style={{ background: '#ff8496', color: '#1c1529' }}>Stop recording</button> : <><button className="primary-button" onClick={() => void start()}>Start recording</button><button className="quiet-button" onClick={() => void evaluate()}>Check rules</button></>}<span className={`capture-status ${active ? 'live' : ''}`}>{active ? '● Live' : 'Ready'}</span></div>{decision && <span className="ingestion-message">Rule engine: {decision.allowed ? `${decision.action} — ${decision.reason}` : `blocked — ${decision.reason}`}</span>}{message && <span className="ingestion-message">{message}</span>}{active && <div style={{ marginTop: '20px', padding: '20px', border: '1px solid #493b60', borderRadius: '8px', background: '#1c1928' }}><span className="eyebrow accent">Meeting visualization</span><div style={{ display: 'flex', gap: '12px', marginTop: '12px', alignItems: 'center' }}><div style={{ width: '60px', height: '60px', borderRadius: '50%', background: 'linear-gradient(135deg, #bb9cff, #6f5b9f)', display: 'grid', placeItems: 'center', animation: 'pulse 2s infinite' }}>🎤</div><div style={{ fontSize: '11px', color: 'var(--soft)' }}><strong style={{ fontSize: '14px', display: 'block' }}>Active conversation</strong><span style={{ color: 'var(--muted)', marginTop: '4px', display: 'block' }}>Participants will be labeled during transcription</span></div></div></div>}</div></section>
}

function Data({ workspace }: { workspace: Workspace }) {
  const [path, setPath] = useState('exports/neural-workspace.json')
  const [message, setMessage] = useState('')
  const [importPath, setImportPath] = useState('exports/neural-workspace.json')
  const [deletion, setDeletion] = useState<WorkspaceDeletionPreview | null>(null)
  const exportData = async () => { try { const output = await exportWorkspace(workspace.id, path); setMessage(`Workspace exported to ${output}.`) } catch (error) { setMessage(errMsg(error)) } }
  const importData = async () => { try { const id = await importWorkspace(importPath); setMessage(`Workspace ${id} imported without replacing existing records.`) } catch (error) { setMessage(errMsg(error)) } }
  const inspectDeletion = async () => { try { setDeletion(await previewWorkspaceDeletion(workspace.id)) } catch (error) { setMessage(errMsg(error)) } }
  const removeWorkspace = async () => { if (!deletion || !await nativeConfirm(`Delete ${deletion.workspace_name} and all listed derived data? This cannot be undone.`)) return; try { await deleteWorkspace(workspace.id); setMessage('Workspace deleted. Restart or select another workspace.') } catch (error) { setMessage(errMsg(error)) } }
  const [context, setContext] = useState('')
  const exportCtx = async () => { setMessage('Compiling workspace context...'); try { const markdown = await exportContext(workspace.id); setContext(markdown); setMessage('Context export ready below (markdown).') } catch (error) { setMessage(errMsg(error)) } }
  return <section className="data-section"><div className="empty-section task-intro"><span className="eyebrow accent">Local portability</span><h2>Keep your context<br /><em>movable.</em></h2><p>Export structured workspace data for backup, migration, or inspection. Recordings and large source files remain at their original paths.</p></div><div className="data-panel"><span className="eyebrow">Workspace export</span><h3>{workspace.name}</h3><label>Output JSON path<input value={path} onChange={(event) => setPath(event.target.value)} /></label><button className="primary-button" onClick={() => void exportData()}>Export workspace</button><span className="eyebrow">Restore export</span><label>Input JSON path<input value={importPath} onChange={(event) => setImportPath(event.target.value)} /></label><button className="quiet-button" onClick={() => void importData()}>Import workspace</button><span className="eyebrow">Context export</span><p>Markdown bundle of this workspace for coding agents (Claude Code, Codex, OpenCode).</p><button className="quiet-button" onClick={() => void exportCtx()}>Export context</button>{context && <pre style={{ whiteSpace: 'pre-wrap', fontSize: '11px', lineHeight: 1.6, maxHeight: '320px', overflow: 'auto', border: '1px solid var(--line)', borderRadius: '8px', padding: '12px' }}>{context}</pre>}<span className="eyebrow">Deletion review</span><p>Review related records before removing this workspace.</p><button className="quiet-button" onClick={() => void inspectDeletion()}>Preview deletion</button>{deletion && <div className="deletion-preview"><strong>{deletion.workspace_name}</strong><span>{deletion.sources} sources · {deletion.meetings} meetings · {deletion.transcript_segments} transcript segments</span><span>{deletion.tasks} tasks · {deletion.reminders} reminders · {deletion.notes} notes · {deletion.emails} emails · {deletion.agents} agents</span>{deletion.recordings.length > 0 && <span>{deletion.recordings.length} recording files remain at their original paths.</span>}<button className="quiet-button" onClick={() => void removeWorkspace()}>Delete workspace and database records</button></div>}{message && <span className="ingestion-message">{message}</span>}</div></section>
}

function Meetings({ workspace }: { workspace: Workspace }) {
  const [path, setPath] = useState('')
  const [title, setTitle] = useState('Imported meeting')
  const [meetings, setMeetings] = useState<Meeting[]>([])
  const [message, setMessage] = useState('')
  const [working, setWorking] = useState(false)
  const [transcript, setTranscript] = useState<{ meeting: Meeting; segments: TranscriptSegment[] } | null>(null)
  const [summary, setSummary] = useState('')
  const [summaryActions, setSummaryActions] = useState<MeetingSummary['actions']>([])
  const [detected, setDetected] = useState<DetectedMeeting[]>([])
  const [voiceProfiles, setVoiceProfiles] = useState<VoiceProfile[]>([])
  useEffect(() => {
    const refresh = () => { void listMeetings(workspace.id).then(setMeetings).catch(() => setMeetings([])); void listVoiceProfiles(workspace.id).then(setVoiceProfiles).catch(() => setVoiceProfiles([])) }
    refresh()
    // Auto-pipeline: recordings import, transcribe, and summarize in the
    // background — poll so queue progress shows up without manual refresh.
    const timer = window.setInterval(refresh, 10000)
    return () => window.clearInterval(timer)
  }, [workspace.id])
  const detect = async () => { try { setDetected(await detectUpcomingMeetings(workspace.id, 48)) } catch (error) { setMessage(errMsg(error)) } }
  const importRecording = async () => { if (!path.trim()) return setMessage('Enter a local recording path.'); setWorking(true); try { const meeting = await importMeetingRecording(workspace.id, path.trim(), title.trim() || 'Imported meeting'); setMeetings((current) => [meeting, ...current]); setPath(''); setMessage('Recording registered and ready for transcription.') } catch (error) { setMessage(errMsg(error)) } finally { setWorking(false) } }
  const transcribe = async (meeting: Meeting) => { setWorking(true); try { const job = await queueTranscription(meeting.id); setMeetings((current) => current.map((item) => item.id === meeting.id ? { ...item, status: 'transcription_queued' } : item)); setMessage(`Transcription queued for ${meeting.title}.`); await processTranscriptionJob(job.id); setMeetings((current) => current.map((item) => item.id === meeting.id ? { ...item, status: 'transcribed' } : item)); setMessage(`${meeting.title} transcription completed.`) } catch (error) { setMessage(errMsg(error)) } finally { setWorking(false) } }
  const openTranscript = async (meeting: Meeting) => { setWorking(true); try { setTranscript({ meeting, segments: await listTranscriptSegments(meeting.id) }) } catch (error) { setMessage(errMsg(error)) } finally { setWorking(false) } }
  const summarize = async (meeting: Meeting) => { setWorking(true); setMessage('Asking the local model to summarize...'); try { const result = await summarizeMeeting(meeting.id); setSummary(`${result.summary}${result.actions.length ? `\n\nAction items\n${result.actions.map((action) => `- ${action.title}`).join('\n')}` : ''}`); setSummaryActions(result.actions); setMessage(result.actions.length ? `${result.actions.length} action items saved locally.` : 'Summary saved locally.') } catch (error) { setMessage(errMsg(error)) } finally { setWorking(false) } }
  const promote = async (action: MeetingSummary['actions'][number]) => { try { await promoteAction(action.id, workspace.id); setSummaryActions((current) => current.map((item) => item.id === action.id ? { ...item, status: 'promoted' } : item)); setMessage('Action promoted to the workspace task list.') } catch (error) { setMessage(errMsg(error)) } }
  const renameSpeaker = async (segment: TranscriptSegment, speaker: string) => { const updated = { ...segment, speaker }; setTranscript((current) => current ? { ...current, segments: current.segments.map((item) => item.id === segment.id ? updated : item) } : current); try { await updateTranscriptSpeaker(segment.id, speaker); const existing = voiceProfiles.find((p) => p.speaker_label.toLowerCase() === speaker.toLowerCase()); if (existing) { await linkTranscriptToProfile(segment.id, existing.id); setVoiceProfiles((cur) => cur.map((p) => p.id === existing.id ? { ...p, sample_count: p.sample_count + 1 } : p)) } else if (speaker.trim()) { const profile = await createVoiceProfile(workspace.id, speaker.trim()); setVoiceProfiles((cur) => [...cur, profile]); await linkTranscriptToProfile(segment.id, profile.id) } } catch { setMessage('Speaker label could not be saved.') } }
  const editSegmentText = async (segment: TranscriptSegment, text: string) => { setTranscript((current) => current ? { ...current, segments: current.segments.map((item) => item.id === segment.id ? { ...item, text } : item) } : current); try { await updateTranscriptSegmentText(segment.id, text) } catch (error) { setMessage(errMsg(error)) } }
  const reprocess = async (meeting: Meeting) => { setWorking(true); try { const job = await reprocessTranscription(meeting.id); setMessage(`Transcription re-queued for ${meeting.title} (${job.status}).`) } catch (error) { setMessage(errMsg(error)) } finally { setWorking(false) } }
  const [batchPaths, setBatchPaths] = useState('')
  const [botRuns, setBotRuns] = useState<BotRun[]>([])
  useEffect(() => { void import('./data/botRuns').then((m) => m.listBotRuns(workspace.id)).then(setBotRuns).catch(() => setBotRuns([])) }, [workspace.id])
  const batchImport = async () => { const paths = batchPaths.split('\n').map((p) => p.trim()).filter(Boolean); if (!paths.length) return setMessage('Enter at least one recording path (one per line).'); setWorking(true); try { const imported = await importMeetingRecordingsBatch(workspace.id, paths); setMeetings((current) => [...imported, ...current]); setBatchPaths(''); setMessage(`${imported.length} recording${imported.length === 1 ? '' : 's'} imported.`) } catch (error) { setMessage(errMsg(error)) } finally { setWorking(false) } }
  const removeProfile = async (profile: VoiceProfile) => { if (!await nativeConfirm(`Delete voice profile "${profile.speaker_label}"?`)) return; try { await deleteVoiceProfile(profile.id); setVoiceProfiles((cur) => cur.filter((p) => p.id !== profile.id)); setMessage('Voice profile deleted.') } catch (error) { setMessage(errMsg(error)) } }
  return <section className="meetings-section"><div className="meetings-header"><div><span className="eyebrow accent">Meeting intelligence</span><h2>Every conversation,<br /><em>remembered.</em></h2><p>Import a local recording or detect upcoming calendar meetings. Transcription and speaker diarization will process meetings marked ready.</p></div><div className="meeting-wave">∿</div></div><div className="ingestion-panel"><span className="eyebrow">Calendar detection</span><h3>Upcoming meetings</h3><p>Scans your calendar events for meetings within the next 48 hours.</p><div style={{ marginTop: '15px', display: 'flex', gap: '10px', alignItems: 'center' }}><button className="primary-button" disabled={working} onClick={() => void detect()}>Scan calendar</button>{detected.length > 0 && <span className="pill pill-local">{detected.length} meeting{detected.length === 1 ? '' : 's'} found</span>}</div>{detected.length > 0 && <div style={{ marginTop: '12px', display: 'grid', gap: '6px' }}>{detected.map((d) => <div key={d.event_id} style={{ padding: '8px 12px', border: '1px solid var(--line)', borderRadius: '6px', fontSize: '11px' }}><strong>{d.title}</strong><span style={{ color: 'var(--muted)', marginLeft: '10px', font: '10px DM Mono' }}>{new Date(d.starts_at).toLocaleString()}</span>{d.meeting_url && <code style={{ display: 'block', marginTop: '4px', color: 'var(--accent)', font: '9px DM Mono', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{d.meeting_url}</code>}</div>)}</div>}</div><div className="ingestion-panel"><span className="eyebrow">Voice profiles</span><h3>Speaker identities</h3><p>Profiles are created when you label speakers. Sample counts grow with each correction.</p><div style={{ marginTop: '12px', display: 'flex', gap: '8px', flexWrap: 'wrap', alignItems: 'center' }}>{voiceProfiles.length === 0 ? <span className="empty-list">No voice profiles yet. Label speakers in transcripts.</span> : voiceProfiles.map((p) => <span key={p.id} style={{ display: 'flex', alignItems: 'center', gap: '4px' }}><span className="pill pill-local" title={`${p.sample_count} samples`}>{p.speaker_label} · {p.sample_count}</span><button className="quiet-button" onClick={() => void removeProfile(p)} title="Delete voice profile" style={{ fontSize: '10px', padding: '2px 5px' }}>✕</button></span>)}</div></div>{botRuns.length > 0 && <div className="ingestion-panel"><span className="eyebrow">Teams bot</span><h3>Recent bot activity</h3><div style={{ display: 'grid', gap: '4px', fontSize: '11px' }}>{botRuns.slice(0, 5).map((run) => <div key={run.id} style={{ display: 'flex', gap: '8px', color: 'var(--soft)' }}><span className="pill pill-local" style={{ flexShrink: 0 }}>{run.status}</span><span>{run.message ?? run.meeting_id ?? run.bot_id}</span></div>)}</div></div>}<div className="ingestion-panel"><span className="eyebrow">Local recording</span><h3>Register audio or video</h3><div className="meeting-form"><input value={title} onChange={(event) => setTitle(event.target.value)} placeholder="Meeting title" /><input value={path} onChange={(event) => setPath(event.target.value)} placeholder="/Users/you/recordings/meeting.m4a" onKeyDown={(event) => { if (event.key === 'Enter') void importRecording() }} /><button className="primary-button" disabled={working} onClick={() => void importRecording()}>{working ? 'Importing...' : 'Import recording'}</button></div><span className="eyebrow">Batch import</span><p>Add multiple recordings at once (one path per line).</p><textarea value={batchPaths} onChange={(event) => setBatchPaths(event.target.value)} placeholder={'/Users/you/recordings/a.m4a\n/Users/you/recordings/b.wav'} rows={2} style={{ width: '100%', background: 'transparent', border: '1px solid var(--line)', borderRadius: '6px', color: 'inherit', padding: '8px', font: '11px DM Mono' }} /><button className="quiet-button" disabled={working} onClick={() => void batchImport()}>Import batch</button>{message && <span className="ingestion-message">{message}</span>}</div><div className="meeting-list">{meetings.length === 0 ? <div className="empty-list">No recordings registered in this workspace yet.</div> : meetings.map((meeting) => <article className="meeting-item" key={meeting.id}><span className="activity-icon violet">◉</span><div><strong>{meeting.title}</strong><span>{meeting.recording_path}</span></div>{meeting.status === 'ready_for_transcription' ? <button className="quiet-button" disabled={working} onClick={() => void transcribe(meeting)}>Queue transcription ↗</button> : meeting.status === 'transcribed' ? <><button className="quiet-button" disabled={working} onClick={() => void openTranscript(meeting)}>Read transcript ↗</button><button className="quiet-button" disabled={working} onClick={() => void summarize(meeting)}>Summarize ↗</button><button className="quiet-button" disabled={working} onClick={() => void reprocess(meeting)} title="Re-queue transcription with a different provider">Reprocess ↗</button></> : <span className="pill pill-local">{meeting.status.replaceAll('_', ' ')}</span>}</article>)}</div>{summary && <div className="summary-panel"><span className="eyebrow">Local model summary</span><div>{summary}</div></div>}{summaryActions.length > 0 && <div style={{ marginTop: '12px' }}>{summaryActions.map((action) => <div key={action.id} style={{ display: 'flex', alignItems: 'center', gap: '8px', padding: '6px 0', fontSize: '11px' }}><span style={{ flex: 1 }}>{action.title}</span><button className="quiet-button" onClick={() => void promote(action)} style={{ fontSize: '10px' }}>{action.status === 'promoted' ? '✓ Promoted' : 'Promote to task'}</button></div>)}</div>}{transcript && <div className="transcript-panel"><div className="section-heading"><div><span className="eyebrow">Transcript</span><h3>{transcript.meeting.title}</h3></div><button className="quiet-button" onClick={() => setTranscript(null)}>Close</button></div>{transcript.segments.length === 0 ? <div className="empty-list">No transcript segments found.</div> : transcript.segments.map((segment) => <div className="transcript-line" key={segment.id}><time>{formatTime(segment.start_seconds)}</time><div><input className="speaker-input" value={segment.speaker ?? ''} placeholder="Speaker name" onChange={(event) => void renameSpeaker(segment, event.target.value)} /><textarea className="segment-text" value={segment.text} rows={2} onChange={(event) => void editSegmentText(segment, event.target.value)} style={{ width: '100%', background: 'transparent', border: '1px solid var(--line)', borderRadius: '6px', color: 'inherit', padding: '6px', font: '12px system-ui', marginTop: '4px' }} /></div></div>)}</div>}</section>
}

/** Native confirm via the Tauri dialog plugin (window.confirm is not
 *  supported in WKWebView and silently returns false). */
async function nativeConfirm(text: string): Promise<boolean> {
  try { return await ask(text, { title: 'Neural Agent OS', kind: 'warning' }) } catch { return await nativeConfirm(text) }
}

/** Native alert via the Tauri dialog plugin. */
async function nativeAlert(text: string): Promise<void> {
  try { await message(text, { title: 'Neural Agent OS' }) } catch { void nativeAlert(text) }
}

function errMsg(error: unknown): string {
  if (error instanceof Error) return error.message
  if (typeof error === 'object' && error !== null) {
    const candidate = (error as { message?: unknown }).message
    if (typeof candidate === 'string' && candidate) return candidate
  }
  return String(error)
}

function formatTime(seconds: number) { const minutes = Math.floor(seconds / 60); const remaining = Math.floor(seconds % 60).toString().padStart(2, '0'); return `${minutes}:${remaining}` }

function Tasks({ workspace }: { workspace: Workspace }) {
  const [tasks, setTasks] = useState<Task[]>([])
  const [reminders, setReminders] = useState<Reminder[]>([])
  const [selectedTask, setSelectedTask] = useState('')
  const [scheduledAt, setScheduledAt] = useState('')
  const [newTaskTitle, setNewTaskTitle] = useState('')
  const [newTaskDue, setNewTaskDue] = useState('')
  const [message, setMessage] = useState('')
  useEffect(() => { void Promise.all([listTasks(workspace.id), listReminders(workspace.id)]).then(([loadedTasks, loadedReminders]) => { setTasks(loadedTasks); setReminders(loadedReminders); setSelectedTask(loadedTasks[0]?.id ?? '') }).catch(() => { setTasks([]); setReminders([]) }) }, [workspace.id])
  useEffect(() => {
    const checkReminders = () => { const now = Date.now(); reminders.filter((reminder) => reminder.status === 'scheduled' && new Date(reminder.scheduled_at).getTime() <= now && new Date(reminder.scheduled_at).getTime() > now - 60000).forEach((reminder) => { void notifyReminder(reminder) }) }
    const timer = window.setInterval(checkReminders, 30000)
    return () => window.clearInterval(timer)
  }, [reminders])
  const addTask = async () => { if (!newTaskTitle.trim()) return setMessage('Enter a task title.'); try { const task = await createTask(workspace.id, newTaskTitle.trim(), newTaskDue ? new Date(newTaskDue).toISOString() : undefined); setTasks((current) => [task, ...current]); setSelectedTask(task.id); setNewTaskTitle(''); setNewTaskDue(''); setMessage('Task created.') } catch (error) { setMessage(errMsg(error)) } }
  const complete = async (task: Task) => { await updateTaskStatus(task.id, task.status === 'completed' ? 'open' : 'completed'); setTasks((current) => current.map((item) => item.id === task.id ? { ...item, status: item.status === 'completed' ? 'open' : 'completed' } : item)) }
  const remove = async (task: Task) => { if (!await nativeConfirm(`Delete task "${task.title}"?`)) return; try { await deleteTask(task.id); setTasks((current) => current.filter((item) => item.id !== task.id)); setMessage('Task deleted.') } catch (error) { setMessage(errMsg(error)) } }
  const schedule = async () => { if (!selectedTask || !scheduledAt) return setMessage('Choose a task and time first.'); const task = tasks.find((item) => item.id === selectedTask); if (!task) return; try { const reminder = await scheduleTaskReminder(workspace.id, task.id, task.title, new Date(scheduledAt).toISOString()); setReminders((current) => [...current, reminder].sort((a, b) => a.scheduled_at.localeCompare(b.scheduled_at))); setMessage('Reminder scheduled locally.'); setScheduledAt('') } catch (error) { setMessage(errMsg(error)) } }
  const cancelRem = async (reminder: Reminder) => { try { await cancelReminder(reminder.id); setReminders((current) => current.map((r) => r.id === reminder.id ? { ...r, status: 'cancelled' } : r)); setMessage('Reminder cancelled.') } catch (error) { setMessage(errMsg(error)) } }
  const deleteRem = async (reminder: Reminder) => { if (!await nativeConfirm('Delete this reminder permanently?')) return; try { await deleteReminder(reminder.id); setReminders((current) => current.filter((r) => r.id !== reminder.id)); setMessage('Reminder deleted.') } catch (error) { setMessage(errMsg(error)) } }
  return <section className="tasks-section"><div className="empty-section task-intro"><span className="eyebrow accent">Follow-through</span><h2>Keep the important<br /><em>things moving.</em></h2><p>Create tasks directly or promote them from meeting action items. Scheduled agents will use this same task stream for reminders and follow-up.</p></div><div className="ingestion-panel"><span className="eyebrow">New task</span><div className="agent-form"><input value={newTaskTitle} onChange={(e) => setNewTaskTitle(e.target.value)} placeholder="Task title" onKeyDown={(e) => { if (e.key === 'Enter') void addTask() }} /><input type="datetime-local" value={newTaskDue} onChange={(e) => setNewTaskDue(e.target.value)} title="Optional due date" /><button className="primary-button" onClick={() => void addTask()}>Add task</button></div>{message && <span className="ingestion-message">{message}</span>}</div><div className="task-list">{tasks.length === 0 ? <div className="empty-list">No tasks yet. Create one above or summarize a transcribed meeting to extract follow-ups.</div> : tasks.map((task) => <div className={`task-item ${task.status === 'completed' ? 'done' : ''}`} key={task.id} style={{ display: 'flex', alignItems: 'center', gap: '8px' }}><button className={`task-item ${task.status === 'completed' ? 'done' : ''}`} style={{ flex: 1, margin: 0, border: 'none', background: 'transparent', padding: 0 }} onClick={() => void complete(task)}><span className="task-check">{task.status === 'completed' ? '✓' : ''}</span><span>{task.title}</span><small>{task.status}{task.due_at ? ` · due ${new Date(task.due_at).toLocaleDateString()}` : ''}</small></button><button className="quiet-button" title="Delete task" onClick={() => void remove(task)} style={{ fontSize: '11px' }}>✕</button></div>)}<div className="reminder-panel"><span className="eyebrow">Local reminders</span><h3>Schedule follow-up</h3><div className="reminder-form"><select value={selectedTask} onChange={(event) => setSelectedTask(event.target.value)}><option value="">Choose a task</option>{tasks.filter((task) => task.status === 'open').map((task) => <option key={task.id} value={task.id}>{task.title}</option>)}</select><input type="datetime-local" value={scheduledAt} onChange={(event) => setScheduledAt(event.target.value)} /><button className="primary-button" onClick={() => void schedule()}>Schedule</button></div>{reminders.length > 0 && <div className="reminder-list">{reminders.map((reminder) => <div className="reminder-item" key={reminder.id} style={{ display: 'flex', alignItems: 'center', gap: '8px' }}><span>◷</span><div style={{ flex: 1 }}><strong>{reminder.title}</strong><small>{new Date(reminder.scheduled_at).toLocaleString()} · <span style={{ color: reminder.status === 'cancelled' ? 'var(--muted)' : 'inherit' }}>{reminder.status}</span></small></div>{reminder.status === 'scheduled' && <button className="quiet-button" onClick={() => void cancelRem(reminder)} title="Cancel reminder" style={{ fontSize: '10px' }}>Cancel</button>}<button className="quiet-button" onClick={() => void deleteRem(reminder)} title="Delete reminder" style={{ fontSize: '10px' }}>✕</button></div>)}</div>}</div></div></section>
}

function ActionQueue({ workspace }: { workspace: Workspace }) {
  const [actions, setActions] = useState<OpenAction[]>([])
  const [message, setMessage] = useState('')
  useEffect(() => { void listOpenActions(workspace.id).then(setActions).catch(() => setActions([])) }, [workspace.id])
  const promote = async (action: OpenAction) => { try { await promoteAction(action.id, workspace.id); setActions((current) => current.filter((item) => item.id !== action.id)); setMessage('Action promoted to a workspace task.') } catch (error) { setMessage(errMsg(error)) } }
  return <section className="action-queue"><div className="empty-section task-intro"><span className="eyebrow accent">Follow-through queue</span><h2>Turn intention<br /><em>into motion.</em></h2><p>These action items came from meeting summaries. Promote one to make it visible in Tasks and reminders.</p>{message && <span className="ingestion-message">{message}</span>}</div><div className="action-list">{actions.length === 0 ? <div className="empty-list">No open action items. Summarize a transcribed meeting to create follow-ups.</div> : actions.map((action) => <article className="action-item" key={action.id}><div><span className="eyebrow">Meeting action</span><strong>{action.title}</strong></div><button className="primary-button" onClick={() => void promote(action)}>Promote to task</button></article>)}</div></section>
}

function Agents({ workspace }: { workspace: Workspace }) {
  const [agents, setAgents] = useState<Agent[]>([])
  const [runs, setRuns] = useState<Record<string, AgentRun[]>>({})
  const [name, setName] = useState('Daily briefing')
  const [prompt, setPrompt] = useState('Prepare a concise briefing from my recent meetings, tasks, and email.')
  const [schedule, setSchedule] = useState('0 8 * * *')
  const [permissionMode, setPermissionMode] = useState<'approval_required' | 'read_only' | 'full'>('approval_required')
  const [editingAgent, setEditingAgent] = useState<Agent | null>(null)
  const [message, setMessage] = useState('')
  useEffect(() => { void listAgents(workspace.id).then(setAgents).catch(() => setAgents([])) }, [workspace.id])
  useEffect(() => { void Promise.all(agents.map(async (agent) => [agent.id, await listAgentRuns(agent.id)] as const)).then((items) => setRuns(Object.fromEntries(items))).catch(() => setRuns({})) }, [agents])
  const resetForm = () => { setEditingAgent(null); setName('Daily briefing'); setPrompt('Prepare a concise briefing from my recent meetings, tasks, and email.'); setSchedule('0 8 * * *'); setPermissionMode('approval_required') }
  const startEdit = (agent: Agent) => { setEditingAgent(agent); setName(agent.name); setPrompt(agent.prompt); setSchedule(agent.schedule); setPermissionMode(agent.permission_mode as 'approval_required' | 'read_only' | 'full') }
  const save = async () => {
    try {
      if (editingAgent) {
        const updated = await updateAgent(editingAgent.id, name, prompt, schedule, permissionMode)
        setAgents((current) => current.map((a) => a.id === updated.id ? updated : a))
        setMessage('Agent updated.')
      } else {
        const agent = await createAgent(workspace.id, name, prompt, schedule, permissionMode)
        setAgents((current) => [agent, ...current])
        setMessage('Agent saved in paused mode.')
      }
      resetForm()
    } catch (error) { setMessage(errMsg(error)) }
  }
  const toggle = async (agent: Agent) => { const status = agent.status === 'active' ? 'paused' : 'active'; await updateAgentStatus(agent.id, status); setAgents((current) => current.map((item) => item.id === agent.id ? { ...item, status } : item)) }
  const run = async (agent: Agent) => { setMessage(`Running ${agent.name} locally...`); try { const result = await runAgentNow(agent.id); setMessage(result.output ? `${agent.name}: ${result.output.slice(0, 140)}` : `${agent.name} completed.`) } catch (error) { setMessage(errMsg(error)) } }
  const createDailyBriefing = () => { setName('Daily briefing'); setPrompt('Prepare a concise briefing covering:\n1. Upcoming meetings for today\n2. Open tasks and their due dates\n3. Recent meeting highlights\n4. Unread emails needing attention\n5. Suggested priorities for the day'); setSchedule('0 8 * * *'); setPermissionMode('read_only') }
  return <section className="agents-section"><div className="empty-section task-intro"><span className="eyebrow accent">Autonomous work</span><h2>Let Neural<br /><em>stay ahead.</em></h2><p>Agents are scheduled workflows with explicit approval by default. Create one here, review it, then activate it.</p></div><div className="agent-panel"><span className="eyebrow">{editingAgent ? `Edit agent: ${editingAgent.name}` : 'New agent'}</span><div className="agent-form"><input value={name} onChange={(event) => setName(event.target.value)} placeholder="Agent name" /><input value={schedule} onChange={(event) => setSchedule(event.target.value)} placeholder="0 8 * * *" /><div style={{ display: 'flex', gap: '8px' }}><select value={permissionMode} onChange={(event) => setPermissionMode(event.target.value as typeof permissionMode)} style={{ flex: 1, border: '1px solid var(--line)', borderRadius: '5px', padding: '10px', color: 'var(--soft)', background: '#10131a', fontSize: '11px' }}><option value="approval_required">Approval required</option><option value="read_only">Read only</option><option value="full">Full access</option></select><button className="quiet-button" onClick={createDailyBriefing} title="Daily briefing template">📋 Template</button></div><textarea value={prompt} onChange={(event) => setPrompt(event.target.value)} /><div style={{ display: 'flex', gap: '8px' }}><button className="primary-button" onClick={() => void save()}>{editingAgent ? 'Update agent' : 'Save paused'}</button>{editingAgent && <button className="quiet-button" onClick={resetForm}>Cancel edit</button>}</div></div>{message && <span className="ingestion-message">{message}</span>}<div className="agent-list">{agents.length === 0 ? <div className="empty-list">No autonomous agents configured.</div> : agents.map((agent) => <article className="agent-item" key={agent.id}><div><strong>{agent.name}</strong><span>{agent.schedule} · {agent.permission_mode}</span></div><button className="quiet-button" onClick={() => void run(agent)}>Run now</button><button className="quiet-button" onClick={() => startEdit(agent)}>Edit</button><button className="quiet-button" onClick={() => void toggle(agent)}>{agent.status === 'active' ? 'Pause' : 'Activate'}</button><span className="pill pill-local">{agent.status}</span></article>)}</div></div></section>
}

function Models({ workspace }: { workspace: Workspace }) {
  const capabilities = ['chat', 'transcription', 'embeddings', 'speech', 'summarization']
  const defaults: Record<string, [string, string]> = { chat: ['Ollama', 'qwen3:14b'], transcription: ['Local', 'whisper-large-v3'], embeddings: ['Ollama', 'nomic-embed-text'], speech: ['OpenAI', 'tts-1'], summarization: ['Ollama', 'qwen3:14b'] }
  const [routes, setRoutes] = useState<ModelRoute[]>([])
  const [message, setMessage] = useState('')
  useEffect(() => { void listModelRoutes(workspace.id).then(setRoutes).catch(() => setRoutes([])) }, [workspace.id])
  const save = async (capability: string, provider: string, model: string) => { try { const route = await setModelRoute(workspace.id, capability, provider, model); setRoutes((current) => [...current.filter((item) => item.capability !== capability), route]); setMessage(`${capability} now uses ${model}.`) } catch (error) { setMessage(errMsg(error)) } }
  const [limits, setLimits] = useState<Record<string, number>>({})
  useEffect(() => { void Promise.all(capabilities.map(async (cap) => { try { const limit = await getTokenLimit(workspace.id, cap); if (limit != null) setLimits((current) => ({ ...current, [cap]: limit })) } catch { /* not configured */ } })) }, [workspace.id])
  const saveLimit = async (capability: string, limitTokens: number) => { try { await setTokenLimit(workspace.id, capability, limitTokens); setLimits((current) => ({ ...current, [capability]: limitTokens })); setMessage(`${capability} token limit set to ${limitTokens.toLocaleString()}.`) } catch (error) { setMessage(errMsg(error)) } }
  const [apiKeys, setApiKeys] = useState<Record<string, string>>({})
  const saveApiKey = async (providerId: string, value: string) => { if (!value.trim()) return setMessage('Enter an API key first.'); try { await storeProviderSecret(providerId, 'api_key', value.trim()); setApiKeys((current) => ({ ...current, [providerId]: value.trim() })); setMessage(`${providerId} API key saved to the OS keychain.`) } catch (error) { setMessage(errMsg(error)) } }
  const clearApiKey = async (providerId: string) => { try { await deleteProviderSecret(providerId, 'api_key'); setApiKeys((current) => ({ ...current, [providerId]: '' })); setMessage(`${providerId} API key removed from the keychain.`) } catch (error) { setMessage(errMsg(error)) } }
  return <section className="models-section"><div className="empty-section task-intro"><span className="eyebrow accent">Model control</span><h2>Choose the mind<br /><em>behind each skill.</em></h2><p>Routes are isolated by workspace. Keep sensitive capabilities local or select configured cloud providers when needed.</p>{message && <span className="ingestion-message">{message}</span>}</div><div className="model-list">{capabilities.map((capability) => { const route = routes.find((item) => item.capability === capability); const [defaultProvider, defaultModel] = defaults[capability]; return <ModelRouteEditor capability={capability} provider={route?.provider ?? defaultProvider} model={route?.model ?? defaultModel} onSave={save} key={capability} /> })}</div><div className="ingestion-panel"><span className="eyebrow">Token limits</span><h3>Per-capability ceilings</h3><p>Local runtimes are free; cloud capabilities respect these token limits on top of cost limits.</p>{capabilities.map((capability) => <div key={capability} style={{ display: 'flex', gap: '8px', alignItems: 'center', margin: '6px 0' }}><span style={{ width: '110px', fontSize: '11px' }}>{capability}</span><input type="number" min={0} value={limits[capability] ?? ''} placeholder="unlimited" onChange={(event) => void saveLimit(capability, Number(event.target.value) || 0)} style={{ width: '140px', background: 'transparent', border: '1px solid var(--line)', borderRadius: '6px', color: 'inherit', padding: '6px' }} /></div>)}</div><div className="ingestion-panel"><span className="eyebrow">Provider runtimes</span><h3>Supported backends</h3><p>Set the base URL for local OpenAI-compatible runtimes (LM Studio, llama.cpp, vLLM) with <code>NEURAL_OPENAI_COMPATIBLE_URL</code>. Cloud providers are audited and gated by the data-sharing settings.</p><div style={{ display: 'flex', flexWrap: 'wrap', gap: '6px', marginTop: '10px' }}>{providerOptions.map((option) => <span key={option.id} className={`pill ${option.kind === 'cloud' ? 'pill-cloud' : 'pill-local'}`} title={option.note}>{option.label}</span>)}</div></div><div className="ingestion-panel"><span className="eyebrow">Cloud API keys</span><h3>Provider credentials</h3><p>Keys are stored in the OS keychain and used by Ask Neural, agents, summaries, and email drafts when a cloud provider is selected. Local runtimes need no key.</p>{providerOptions.filter((option) => option.kind === 'cloud' || option.id === 'openai_compatible').map((option) => <div key={option.id} style={{ display: 'flex', gap: '8px', alignItems: 'center', margin: '8px 0' }}><span style={{ width: '130px', fontSize: '11px' }}>{option.label}</span><input type="password" value={apiKeys[option.id] ?? ''} onChange={(event) => setApiKeys((current) => ({ ...current, [option.id]: event.target.value }))} placeholder="API key" style={{ flex: 1, background: 'transparent', border: '1px solid var(--line)', borderRadius: '6px', color: 'inherit', padding: '7px', font: '11px DM Mono' }} /><button className="quiet-button" onClick={() => void saveApiKey(option.id, apiKeys[option.id] ?? '')}>Save</button>{apiKeys[option.id] && <button className="quiet-button" onClick={() => void clearApiKey(option.id)} title="Remove key">✕</button>}</div>)}</div></section>
}

function ModelRouteEditor({ capability, provider, model, onSave }: { capability: string; provider: string; model: string; onSave: (capability: string, provider: string, model: string) => Promise<void> }) {
  const [nextProvider, setNextProvider] = useState(provider)
  const [nextModel, setNextModel] = useState(model)
  const option = providerOptions.find((p) => p.id === nextProvider) ?? providerOptions.find((p) => p.label.toLowerCase() === nextProvider.toLowerCase())
  return <div className="model-route"><div><span className="eyebrow">{capability}</span><strong>{model}</strong></div><select value={option?.id ?? nextProvider} onChange={(event) => setNextProvider(event.target.value)}>{providerOptions.map((option) => <option key={option.id} value={option.id}>{option.label}{option.kind === 'cloud' ? ' (cloud)' : ''}</option>)}</select><input value={nextModel} onChange={(event) => setNextModel(event.target.value)} /><button className="quiet-button" onClick={() => void onSave(capability, nextProvider, nextModel)}>Save</button></div>
}

function AssistantComposer({ workspace, onClose }: { workspace: Workspace; onClose: () => void }) {
  const [prompt, setPrompt] = useState('')
  const [response, setResponse] = useState('')
  const [working, setWorking] = useState(false)
  const [listening, setListening] = useState(false)
  const [language, setLanguage] = useState<AssistantLanguage>('en')
  const ask = async () => { if (!prompt.trim()) return; setWorking(true); try { const result = await askAssistant(workspace.id, prompt); const sources = result.citations.map((citation) => `- ${citation.title}${citation.uri ? ` (${citation.uri})` : ''}`).join('\n'); setResponse(result.response + (sources ? `\n\nSources\n${sources}` : '')) } catch (error) { setResponse(errMsg(error)) } finally { setWorking(false) } }
  const speak = () => { if (response && 'speechSynthesis' in window) { const utterance = new SpeechSynthesisUtterance(response); utterance.lang = languageSpeechCodes[language]; window.speechSynthesis.speak(utterance) } }
  const listen = async () => {
    type SpeechRecognitionLike = new () => { lang: string; start: () => void; onresult: ((event: { results: ArrayLike<ArrayLike<{ transcript: string }>> }) => void) | null; onend: (() => void) | null }
    const win = window as unknown as { SpeechRecognition?: SpeechRecognitionLike; webkitSpeechRecognition?: SpeechRecognitionLike }
    const RecognitionCtor = win.SpeechRecognition ?? win.webkitSpeechRecognition
    if (RecognitionCtor) {
      // Browser/WebView2 runtime: Web Speech API
      const recognition = new RecognitionCtor()
      recognition.lang = languageSpeechCodes[language]
      recognition.onresult = (event) => setPrompt(Array.from(event.results).map((result) => result[0].transcript).join(' '))
      recognition.onend = () => setListening(false)
      setListening(true)
      recognition.start()
      return
    }
    // Desktop runtime (macOS/Linux WebView): push-to-talk via local Whisper.
    if (listening) {
      setListening(false)
      try {
        const text = await stopVoiceCapture()
        if (text.trim()) setPrompt((current) => (current ? current + ' ' : '') + text.trim())
      } catch (error) { setResponse(errMsg(error)) }
    } else {
      try { await startVoiceCapture(language); setListening(true) } catch (error) { setResponse(errMsg(error)) }
    }
  }
  return <div className="modal-backdrop" onClick={onClose}><section className="composer" onClick={(event) => event.stopPropagation()}><div className="composer-orb">✦</div><span className="eyebrow">Neural assistant · {workspace.name}</span><h2>What should I take care of?</h2><textarea autoFocus value={prompt} onChange={(event) => setPrompt(event.target.value)} placeholder="Ask about a meeting, search your knowledge, or plan a task..." onKeyDown={(event) => { if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) void ask() }} />{response && <div className="assistant-response">{response}</div>}<div className="composer-footer"><span>{working ? 'Thinking locally...' : listening ? 'Listening...' : '⌘ Enter to send'}</span><div><select className="lang-select" value={language} onChange={(event) => setLanguage(event.target.value as AssistantLanguage)}><option value="en">EN</option><option value="hi">हि</option><option value="te">తె</option></select><button className="quiet-button" onClick={() => void listen()}>{listening ? 'Stop' : 'Voice'}</button><button className="quiet-button" disabled={!response} onClick={speak}>Speak</button><button className="quiet-button" onClick={onClose}>Close</button><button className="primary-button" disabled={working} onClick={() => void ask()}>Ask Neural</button></div></div></section></div>
}

function Calendar({ workspace }: { workspace: Workspace }) {
  const [events, setEvents] = useState<CalendarEvent[]>([])
  const [title, setTitle] = useState('')
  const [startsAt, setStartsAt] = useState('')
  const [endsAt, setEndsAt] = useState('')
  const [recurrence, setRecurrence] = useState('')
  const [message, setMessage] = useState('')
  const [connections, setConnections] = useState<CalendarConnection[]>([])
  const [editingId, setEditingId] = useState('')
  const [expanded, setExpanded] = useState<Record<string, CalendarParticipant[]>>({})
  const [inviteName, setInviteName] = useState('')
  const [inviteEmail, setInviteEmail] = useState('')
  const [conflicts, setConflicts] = useState<ConflictInfo[]>([])
  const [participantEvent, setParticipantEvent] = useState('')
  useEffect(() => { void Promise.all([listCalendarEvents(workspace.id), listCalendarConnections(workspace.id)]).then(([loadedEvents, loadedConnections]) => { setEvents(loadedEvents); setConnections(loadedConnections) }).catch(() => { setEvents([]); setConnections([]) }) }, [workspace.id])
  const reload = async () => { setEvents(await listCalendarEvents(workspace.id)); setConnections(await listCalendarConnections(workspace.id)) }
  const resetForm = () => { setEditingId(''); setTitle(''); setStartsAt(''); setEndsAt(''); setRecurrence('') }
  const create = async () => { if (!title || !startsAt || !endsAt) return setMessage('Add a title and both event times.'); try { if (editingId) { await updateCalendarEvent(editingId, title, new Date(startsAt).toISOString(), new Date(endsAt).toISOString(), undefined, recurrence || undefined); setMessage('Event updated.') } else { await createCalendarEvent(workspace.id, title, new Date(startsAt).toISOString(), new Date(endsAt).toISOString(), undefined, recurrence || undefined); setMessage('Event saved to the local calendar.') } resetForm(); await reload() } catch (error) { setMessage(errMsg(error)) } }
  const edit = (event: CalendarEvent) => { setEditingId(event.id); setTitle(event.title); setStartsAt(event.starts_at.slice(0, 16)); setEndsAt(event.ends_at.slice(0, 16)); setRecurrence(event.recurrence ?? '') }
  const remove = async (event: CalendarEvent) => { if (!await nativeConfirm(`Delete ${event.title}?`)) return; try { await deleteCalendarEvent(event.id); setEvents((current) => current.filter((item) => item.id !== event.id)); setMessage('Calendar event deleted.') } catch (error) { setMessage(errMsg(error)) } }
  const checkConflicts = async (event: CalendarEvent) => { try { setConflicts(await checkConflictsForEvent(workspace.id, event.id)) } catch (error) { setMessage(errMsg(error)) } }
  const remindEvent = async (event: CalendarEvent) => { try { const reminder = await scheduleEventReminder(workspace.id, event.id, `Prep for ${event.title}`, new Date(event.starts_at).toISOString()); setMessage(`Reminder "${reminder.title}" scheduled for ${new Date(reminder.scheduled_at).toLocaleString()}.`) } catch (error) { setMessage(errMsg(error)) } }
  const toggleParticipants = async (event: CalendarEvent) => { if (expanded[event.id]) { setExpanded((cur) => { const next = { ...cur }; delete next[event.id]; return next }); return } try { const list = await listParticipants(event.id); setExpanded((cur) => ({ ...cur, [event.id]: list })); setParticipantEvent(event.id) } catch (error) { setMessage(errMsg(error)) } }
  const addInvitee = async (eventId: string) => { if (!inviteName.trim()) return setMessage('Enter a participant name.'); try { await addParticipant(eventId, inviteName.trim(), inviteEmail.trim() || undefined); setInviteName(''); setInviteEmail(''); setExpanded((cur) => ({ ...cur, [eventId]: [...(cur[eventId] ?? []), { id: 'tmp', event_id: eventId, name: inviteName.trim(), email: inviteEmail.trim() || undefined, status: 'invited' }] })); setMessage('Participant added. Use the ✉ button to send an invitation.') } catch (error) { setMessage(errMsg(error)) } }
  const invite = async (eventId: string, participantId: string) => { try { const result = await sendCalendarInvitation(eventId, participantId); setMessage(`Invitation sent to ${result.sent_to}.`) } catch (error) { setMessage(errMsg(error)) } }
  const connect = async (provider: 'google' | 'outlook') => { try { const connection = await connectCalendarProvider(workspace.id, provider, provider === 'google' ? 'Google Calendar' : 'Microsoft Outlook'); setConnections((current) => [...current.filter((item) => item.provider !== provider), connection]); try { const oauth = await generateOAuthUrl(provider); window.open(oauth.url, '_blank'); setMessage(`Authorize ${provider === 'google' ? 'Google' : 'Outlook'} in the opened browser window. After authorization, return here and click Sync to pull events.`) } catch { setMessage(`${provider === 'google' ? 'Google' : 'Outlook'} connection created. Set OAuth credentials in environment to authorize.`) } } catch (error) { setMessage(errMsg(error)) } }
  const syncConnection = async (connection: CalendarConnection) => { setMessage(`Syncing ${connection.provider} calendar...`); try { const result = await syncCalendarEvents(connection.id); await reload(); setMessage(result) } catch (error) { setMessage(errMsg(error)) } }
  const pullRemote = async (connection: CalendarConnection) => { setMessage(`Pulling ${connection.provider} calendar...`); try { const result = await pullRemoteCalendar(connection.id); const [created, updated, conflictCount] = result; await reload(); setMessage(`Pulled: ${created} new, ${updated} updated, ${conflictCount} conflicts`) } catch (error) { setMessage(errMsg(error)) } }
  return <section className="calendar-section"><div className="calendar-header"><div><span className="eyebrow accent">Local calendar</span><h2>Make time for<br /><em>what matters.</em></h2><p>This built-in calendar is the source of truth for local events. Create, edit, invite participants, and sync bidirectionally with Google and Outlook.</p></div><div className="calendar-glyph">▦</div></div><div className="calendar-create"><span className="eyebrow">{editingId ? 'Edit event' : 'New event'}</span><div className="calendar-form"><input value={title} onChange={(event) => setTitle(event.target.value)} placeholder="Event title" /><input type="datetime-local" value={startsAt} onChange={(event) => setStartsAt(event.target.value)} /><input type="datetime-local" value={endsAt} onChange={(event) => setEndsAt(event.target.value)} /><input value={recurrence} onChange={(event) => setRecurrence(event.target.value)} placeholder="Recurrence: 1 week | FREQ=WEEKLY" /><button className="primary-button" onClick={() => void create()}>{editingId ? 'Update event' : 'Add event'}</button>{editingId && <button className="quiet-button" onClick={resetForm}>Cancel edit</button>}</div>{message && <span className="ingestion-message">{message}</span>}</div><div className="calendar-create"><span className="eyebrow">External calendars</span><div className="calendar-connections"><button className="quiet-button" onClick={() => void connect('google')}>+ Connect Google Calendar</button><button className="quiet-button" onClick={() => void connect('outlook')}>+ Connect Outlook</button>{connections.map((connection) => <span key={connection.id} style={{ display: 'flex', alignItems: 'center', gap: '8px' }}><span className="pill pill-local">{connection.provider} · {connection.status}</span><button className="quiet-button" onClick={() => void syncConnection(connection)}>Push</button><button className="quiet-button" onClick={() => void pullRemote(connection)}>Pull</button></span>)}</div></div>{conflicts.length > 0 && <div className="ingestion-panel"><span className="eyebrow">Conflicts detected</span>{conflicts.map((c) => <div key={`${c.event_id}-${c.overlapping_event_id}`} style={{ fontSize: '11px', padding: '6px 0', color: 'var(--soft)' }}>⚠ <strong>{c.title}</strong> overlaps with <strong>{c.overlapping_title}</strong> ({new Date(c.overlapping_starts_at).toLocaleString()})</div>)}</div>}<div className="event-list">{events.length === 0 ? <div className="empty-list">No local events yet.</div> : events.map((event) => <article className="event-item" key={event.id}><time>{new Date(event.starts_at).toLocaleString()}</time><div><strong>{event.title}</strong><span>{new Date(event.starts_at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })} - {new Date(event.ends_at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span>{event.recurrence && <span className="pill pill-local" style={{ marginLeft: '6px' }}>{event.recurrence}</span>}</div><div style={{ display: 'flex', gap: '6px', alignItems: 'center' }}><button className="quiet-button" onClick={() => void edit(event)}>Edit</button><button className="quiet-button" onClick={() => void checkConflicts(event)}>Conflicts</button><button className="quiet-button" onClick={() => void remindEvent(event)} title="Schedule a reminder before this event">Remind</button><button className="quiet-button" onClick={() => void toggleParticipants(event)}>People</button><button className="quiet-button" onClick={() => void remove(event)}>Delete</button><span className="pill pill-local">{event.provider}</span></div>{expanded[event.id] && <div className="participant-panel" style={{ marginTop: '8px', padding: '10px', borderTop: '1px solid var(--line)' }}><div style={{ display: 'flex', gap: '6px', flexWrap: 'wrap' }}><input value={inviteName} onChange={(e) => setInviteName(e.target.value)} placeholder="Name" style={{ width: '110px', border: '1px solid var(--line)', borderRadius: '5px', padding: '6px', background: '#10131a', color: 'var(--soft)', fontSize: '10px' }} /><input value={inviteEmail} onChange={(e) => setInviteEmail(e.target.value)} placeholder="email" style={{ width: '160px', border: '1px solid var(--line)', borderRadius: '5px', padding: '6px', background: '#10131a', color: 'var(--soft)', fontSize: '10px' }} /><button className="quiet-button" onClick={() => void addInvitee(event.id)}>Add</button></div>{(expanded[event.id] ?? []).map((p) => <div key={p.id} style={{ display: 'flex', gap: '8px', alignItems: 'center', padding: '4px 0', fontSize: '11px' }}><span>{p.name}</span><span style={{ color: 'var(--muted)' }}>{p.email ?? ''}</span><span className="pill pill-local">{p.status}</span>{p.id !== 'tmp' && <button className="quiet-button" title="Send invitation" onClick={() => void invite(event.id, p.id)}>✉</button>}</div>)}</div>}</article>)}</div></section>
}

function Mail({ workspace }: { workspace: Workspace }) {
  const [accounts, setAccounts] = useState<EmailAccount[]>([])
  const [provider, setProvider] = useState<'gmail' | 'outlook' | 'imap'>('gmail')
  const [label, setLabel] = useState('')
  const [imapHost, setImapHost] = useState('')
  const [smtpHost, setSmtpHost] = useState('')
  const [message, setMessage] = useState('')
  const [emails, setEmails] = useState<Email[]>([])
  const [query, setQuery] = useState('')
  const [composing, setComposing] = useState(false)
  const [to, setTo] = useState('')
  const [subject, setSubject] = useState('')
  const [body, setBody] = useState('')
  const [selectedAccount, setSelectedAccount] = useState('')
  useEffect(() => { void listEmailAccounts(workspace.id).then(setAccounts).catch(() => setAccounts([])) }, [workspace.id])
  useEffect(() => { void listEmails(workspace.id).then(setEmails).catch(() => setEmails([])) }, [workspace.id])
  const addAccount = async () => { if (!label.trim()) return setMessage('Add an account label or email address.'); try { const account = await connectEmailAccount(workspace.id, provider, label.trim(), imapHost || undefined, smtpHost || undefined); setAccounts((current) => [...current.filter((item) => item.provider !== provider), account]); setMessage(`${provider === 'imap' ? 'IMAP/SMTP' : provider} account saved locally and awaiting authorization.`) } catch (error) { setMessage(errMsg(error)) } }
  const sync = async (account: EmailAccount) => { setMessage(`Syncing ${account.account_label}...`); try { const result = account.provider === 'imap' ? await syncEmailAccount(account.id) : await syncEmailOAuth(account.id); setAccounts((current) => current.map((item) => item.id === account.id ? { ...item, status: 'synced' } : item)); setEmails(await listEmails(workspace.id)); setMessage(`${result.stored} new message${result.stored === 1 ? '' : 's'} stored from ${result.fetched} fetched (${account.provider === 'imap' ? 'IMAP' : 'OAuth'}).`) } catch (error) { setMessage(errMsg(error)) } }
  const search = async () => { setEmails(await listEmails(workspace.id, query)) }
  const [triage, setTriage] = useState<TriageItem[]>([])
  const runTriage = async () => { setMessage('Reviewing recent inbox...'); try { const items = await emailTriage(workspace.id); setTriage(items); setMessage(items.length ? `${items.length} message${items.length === 1 ? '' : 's'} flagged for triage.` : 'No recent messages to triage.') } catch (error) { setMessage(errMsg(error)) } }
  const generateDraft = async () => { if (!subject.trim()) return setMessage('Add a subject to generate a draft from.'); setMessage('Generating draft...'); try { const draft = await generateEmailDraft(workspace.id, `Write an email with subject: ${subject}`, to ? `To: ${to}` : 'General email', 'qwen3:14b'); setBody(draft); setMessage('Draft generated. Review and edit before sending.') } catch (error) { setMessage(errMsg(error)) } }
  const compose = async () => {
    if (!to.trim() || !subject.trim() || !selectedAccount) return setMessage('Recipient, subject, and an account are required.')
    setMessage('Sending...')
    try {
      const result = await sendEmail(selectedAccount, to, subject, body)
      setMessage(result.message)
      if (result.sent) { setComposing(false); setTo(''); setSubject(''); setBody('') }
    } catch (error) { setMessage(errMsg(error)) }
  }
  return <section className="mail-section"><div className="mail-header"><div><span className="eyebrow accent">Private inbox</span><h2>Your attention,<br /><em>well directed.</em></h2><p>Mail stays connected to the selected workspace. Gmail, Outlook, and generic IMAP/SMTP accounts share one local model.</p></div><div className="mail-glyph">✉</div></div><div className="mail-create"><span className="eyebrow">Add account</span><div className="mail-form"><select value={provider} onChange={(event) => setProvider(event.target.value as typeof provider)}><option value="gmail">Gmail</option><option value="outlook">Outlook / Microsoft 365</option><option value="imap">Generic IMAP / SMTP</option></select><input value={label} onChange={(event) => setLabel(event.target.value)} placeholder="you@example.com" />{provider === 'imap' && <><input value={imapHost} onChange={(event) => setImapHost(event.target.value)} placeholder="IMAP host" /><input value={smtpHost} onChange={(event) => setSmtpHost(event.target.value)} placeholder="SMTP host" /></>}<button className="primary-button" onClick={() => void addAccount()}>Add account</button></div>{message && !composing && <span className="ingestion-message">{message}</span>}</div><div className="account-list">{accounts.length === 0 ? <div className="empty-list">No mail accounts connected to this workspace.</div> : accounts.map((account) => <article className="account-item" key={account.id}><span className="activity-icon violet">✉</span><div><strong>{account.account_label}</strong><span>{account.provider}{account.imap_host ? ` · ${account.imap_host}` : ''}</span></div><button className="quiet-button" onClick={() => void sync(account)}>Sync ↗</button><span className="pill pill-local">{account.status}</span></article>)}</div>{!composing && <button className="primary-button" style={{ marginTop: '15px' }} onClick={() => { setComposing(true); setSelectedAccount(accounts[0]?.id ?? '') }}>+ Compose email</button>}{composing && <div className="ingestion-panel"><span className="eyebrow">New message</span><div className="compose-form"><select value={selectedAccount} onChange={(event) => setSelectedAccount(event.target.value)}>{accounts.map((account) => <option key={account.id} value={account.id}>{account.account_label}</option>)}</select><input value={to} onChange={(event) => setTo(event.target.value)} placeholder="To" /><input value={subject} onChange={(event) => setSubject(event.target.value)} placeholder="Subject" /><textarea value={body} onChange={(event) => setBody(event.target.value)} placeholder="Write your message..." /><div style={{ display: 'flex', gap: '9px' }}><button className="primary-button" onClick={() => void compose()}>Send</button><button className="quiet-button" onClick={() => void generateDraft()}>✨ Draft</button><button className="quiet-button" onClick={() => { setComposing(false); setMessage('') }}>Cancel</button></div></div>{message && <span className="ingestion-message">{message}</span>}</div>}<div className="mailbox"><div className="mailbox-search"><span className="eyebrow">Local mailbox</span><div><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search subject, sender, or body" onKeyDown={(event) => { if (event.key === 'Enter') void search() }} /><button className="primary-button" onClick={() => void search()}>Search</button><button className="quiet-button" onClick={() => void runTriage()}>Triage inbox ↗</button></div></div>{triage.length > 0 && <div className="triage-list" style={{ margin: '12px 0' }}><span className="eyebrow">Triage suggestions</span>{triage.map((item) => <div key={item.email_id} style={{ display: 'flex', gap: '10px', padding: '6px 0', borderBottom: '1px solid var(--line)', fontSize: '11px', alignItems: 'center' }}><span className="pill pill-local" style={{ flexShrink: 0 }}>{item.reason}</span><div style={{ flex: 1, minWidth: 0 }}><strong style={{ display: 'block', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{item.subject}</strong><span style={{ color: 'var(--muted)' }}>{item.sender}</span></div></div>)}</div>}{emails.length === 0 ? <div className="empty-list">No synchronized messages in this workspace.</div> : emails.map((email) => <article className="email-item" key={email.id}><div><strong>{email.subject}</strong><span>{email.sender ?? 'Unknown sender'}</span><p>{email.body?.slice(0, 180) ?? ''}</p></div><time>{email.received_at ? new Date(email.received_at).toLocaleDateString() : '—'}</time><button className="quiet-button" title="Extract action items" onClick={() => { void extractEmailActions(email.id).then((tasks) => { setMessage(`${tasks.length} action item${tasks.length === 1 ? '' : 's'} extracted.`); if (tasks.length === 0) setMessage('No action items found in this email.') }).catch((error) => setMessage(errMsg(error))) }}>⚡</button></article>)}</div></section>
}

// ── Diagnostics View ───────────────────────────────────────────────────────

function DiagnosticsView({ workspace }: { workspace: Workspace }) {
  const [stats, setStats] = useState<StorageStats | null>(null)
  const [health, setHealth] = useState<ProviderHealth[]>([])
  const [sync, setSync] = useState<SyncStatus[]>([])
  const [deletion, setDeletion] = useState<FullDeletionPreview | null>(null)
  const [message, setMessage] = useState('')
  const [apiKeys, setApiKeys] = useState<string[]>([])
  const [keyLabel, setKeyLabel] = useState('')
  const [newKey, setNewKey] = useState('')
  const [retentionDays, setRetentionDays] = useState('30')
  useEffect(() => { void getStorageStats().then(setStats).catch(() => setStats(null)) }, [])
  useEffect(() => { void getProviderHealth(workspace.id).then(setHealth).catch(() => setHealth([])) }, [workspace.id])
  useEffect(() => { void getSyncStatus(workspace.id).then(setSync).catch(() => setSync([])) }, [workspace.id])
  useEffect(() => { void listApiKeys().then(setApiKeys).catch(() => setApiKeys([])) }, [])
  const createKey = async () => { if (!keyLabel.trim()) return setMessage('Enter a label for this API key.'); try { const key = await generateApiKey(keyLabel.trim()); setNewKey(key); setApiKeys(await listApiKeys()); setKeyLabel(''); setMessage('API key generated. Copy it now — it is only shown once.') } catch (e) { setMessage(String(e)) } }
  const revokeKey = async (label: string) => { try { await revokeApiKey(label); setApiKeys(await listApiKeys()); setMessage(`API key "${label}" revoked.`) } catch (e) { setMessage(String(e)) } }
  const saveRetention = async () => { try { await setWorkspaceRetention(workspace.id, parseInt(retentionDays)); setMessage(`Recording retention for ${workspace.name} set to ${retentionDays} days.`) } catch (e) { setMessage(String(e)) } }

  return <section className="diagnostics-section">
    <div className="empty-section task-intro"><span className="eyebrow accent">System diagnostics</span><h2>Everything<br /><em>under control.</em></h2><p>Monitor storage, provider health, and sync status. Preview deletion impact before removing data.</p></div>
    <div className="settings-panel">
      <div className="setting-row"><div><strong>Database</strong><span>{stats?.database_path ?? '—'}</span></div><span className="pill pill-local">{stats?.database_size_bytes ? `${(stats.database_size_bytes / 1e6).toFixed(1)} MB` : '—'}</span></div>
      <div className="setting-row"><div><strong>Recordings</strong><span>{stats?.recordings_count ?? 0} files, {(stats?.recordings_size_bytes ?? 0) / 1e6 > 0 ? `${((stats?.recordings_size_bytes ?? 0) / 1e6).toFixed(1)} MB` : '0 MB'}</span></div><span className="pill pill-cloud">{stats?.storage_warning ?? 'OK'}</span></div>
      <div className="setting-row"><div><strong>Sources / Embeddings / Emails</strong></div><span>{stats?.total_sources ?? 0} / {stats?.total_embeddings ?? 0} / {stats?.total_emails ?? 0}</span></div>
    </div>
    <div className="model-list" style={{ marginTop: '18px' }}>
      {health.map((h) => <div className="model-route" key={`${h.provider}-${h.capability}`}><div><span className="eyebrow">{h.capability}</span><strong>{h.model}</strong><span style={{ color: 'var(--muted)', fontSize: '10px' }}>{h.provider} · {h.response_time_ms ? `${h.response_time_ms}ms` : '—'} · {h.consecutive_failures > 0 ? `${h.consecutive_failures} failures` : ''}</span></div><span className={`pill ${h.healthy ? 'pill-local' : 'pill-cloud'}`}>{h.healthy ? 'Healthy' : 'Unhealthy'}</span></div>)}
      {health.length === 0 && <div className="empty-list">No health data yet. Health checks run every 5 minutes.</div>}
    </div>
    <div className="model-list" style={{ marginTop: '18px' }}>
      {sync.map((s) => <div className="model-route" key={s.provider}><div><strong>{s.provider}</strong><span style={{ color: 'var(--muted)', fontSize: '10px' }}>{s.last_sync ? `Last: ${new Date(s.last_sync).toLocaleString()}` : 'Never synced'}</span></div><span style={{ display: 'flex', gap: '6px' }}><span className="pill pill-local">{s.synced} done</span>{s.pending > 0 && <span className="pill pill-cloud">{s.pending} pending</span>}{s.conflicts > 0 && <span className="pill pill-cloud">{s.conflicts} conflicts</span>}{s.failed > 0 && <span className="pill pill-cloud">{s.failed} failed</span>}</span></div>)}
      {sync.length === 0 && <div className="empty-list">No sync providers connected.</div>}
    </div>
    <div className="ingestion-panel" style={{ marginTop: '18px' }}>
      <span className="eyebrow">Deletion impact preview</span>
      <button className="primary-button" onClick={() => { void fullDeletionPreview(workspace.id).then(setDeletion).catch(() => setMessage('Failed to load preview')) }}>Preview full deletion</button>
      {deletion && <div className="summary-panel" style={{ marginTop: '12px' }}>
        <strong>{deletion.workspace_name}</strong>
        <span>{deletion.sources.length} sources · {deletion.meetings.length} meetings · {deletion.notes_count} notes · {deletion.emails_count} emails</span>
        <span>{deletion.total_derived_items} derived items · {(deletion.total_storage_impact_bytes / 1e6).toFixed(1)} MB total impact</span>
      </div>}
      {message && <span className="ingestion-message">{message}</span>}
    </div>
    <div className="ingestion-panel" style={{ marginTop: '18px' }}>
      <span className="eyebrow">MCP / REST API keys</span>
      <p style={{ fontSize: '11px', color: 'var(--muted)', margin: '4px 0 10px' }}>Write operations via REST and MCP require <code>Authorization: Bearer &lt;key&gt;</code>. Read operations stay open on loopback.</p>
      <div style={{ display: 'flex', gap: '8px', alignItems: 'center', marginBottom: '10px' }}>
        <input value={keyLabel} onChange={(e) => setKeyLabel(e.target.value)} placeholder="Label (e.g. claude-code)" style={{ flex: 1, border: '1px solid var(--line)', borderRadius: '5px', padding: '8px', background: '#10131a', color: 'var(--soft)', fontSize: '11px' }} />
        <button className="primary-button" onClick={() => void createKey()}>Generate key</button>
      </div>
      {newKey && <div className="summary-panel" style={{ marginTop: '8px' }}><strong style={{ color: 'var(--green)' }}>Copy this API key now:</strong><code style={{ display: 'block', marginTop: '4px', wordBreak: 'break-all' }}>{newKey}</code></div>}
      {apiKeys.length > 0 && <div>{apiKeys.map((k) => <div key={k} style={{ display: 'flex', gap: '8px', alignItems: 'center', padding: '4px 0', fontSize: '11px' }}><span>{k.replace('api_key_', '')}</span><button className="quiet-button" onClick={() => void revokeKey(k.replace('api_key_', ''))}>Revoke</button></div>)}</div>}
    </div>
    <div className="ingestion-panel" style={{ marginTop: '18px' }}>
      <span className="eyebrow">Recording retention (this workspace)</span>
      <div style={{ display: 'flex', gap: '8px', alignItems: 'center', marginTop: '8px' }}>
        <input value={retentionDays} onChange={(e) => setRetentionDays(e.target.value)} placeholder="Days" type="number" style={{ width: '100px', border: '1px solid var(--line)', borderRadius: '5px', padding: '8px', background: '#10131a', color: 'var(--soft)', fontSize: '11px' }} />
        <button className="primary-button" onClick={() => void saveRetention()}>Save retention</button>
      </div>
      <p style={{ fontSize: '11px', color: 'var(--muted)', margin: '6px 0 0' }}>Recordings older than this are auto-deleted by the scheduler (when autoRetentionCleanup is on).</p>
    </div>
    <div className="ingestion-panel" style={{ marginTop: '18px' }}>
      <span className="eyebrow">Selective derived-data deletion</span>
      <p style={{ fontSize: '11px', color: 'var(--muted)', margin: '4px 0 8px' }}>Remove derived data without deleting the source or recording.</p>
      {deletion?.meetings.map((m) => <div key={m.meeting_id} style={{ display: 'flex', gap: '6px', alignItems: 'center', padding: '4px 0', fontSize: '11px', flexWrap: 'wrap' }}><strong>{m.meeting_title}</strong>{m.transcript_segments > 0 && <button className="quiet-button" onClick={() => { void deleteTranscriptSegments(m.meeting_id).then(() => setMessage(`Transcript removed (${m.meeting_id}).`)).catch((e) => setMessage(String(e))) }}>Delete transcript</button>}{m.summary_exists && <button className="quiet-button" onClick={() => { void deleteMeetingSummary(m.meeting_id).then(() => setMessage('Summary removed.')).catch((e) => setMessage(String(e))) }}>Delete summary</button>}{m.actions > 0 && <button className="quiet-button" onClick={() => { void deleteMeetingActions(m.meeting_id).then(() => setMessage('Action items removed.')).catch((e) => setMessage(String(e))) }}>Delete actions</button>}{m.recording_path && <button className="quiet-button" onClick={() => { void deleteMeetingRecording(m.meeting_id).then(() => setMessage('Recording file removed.')).catch((e) => setMessage(String(e))) }}>Delete recording</button>}</div>)}
      {(!deletion || deletion.meetings.length === 0) && <span className="empty-list">Load the deletion preview above to manage meeting-derived data.</span>}
    </div>
  </section>
}

// ── Approvals View ─────────────────────────────────────────────────────────

function ApprovalsView({ workspace }: { workspace: Workspace }) {
  const [requests, setRequests] = useState<ApprovalRequest[]>([])
  const [message, setMessage] = useState('')
  useEffect(() => { void listPendingApprovals(workspace.id).then(setRequests).catch(() => setRequests([])) }, [workspace.id])
  const approve = async (id: string) => { try { await approveRequest(id, 'user'); setRequests((r) => r.filter((x) => x.id !== id)); setMessage('Approved.') } catch (e) { setMessage(String(e)) } }
  const deny = async (id: string) => { try { await denyRequest(id, 'user', 'Denied by user'); setRequests((r) => r.filter((x) => x.id !== id)); setMessage('Denied.') } catch (e) { setMessage(String(e)) } }
  return <section className="tasks-section">
    <div className="empty-section task-intro"><span className="eyebrow accent">Approval queue</span><h2>Stay in<br /><em>control.</em></h2><p>Agents with approval_required mode need explicit approval before executing dangerous actions.</p>{message && <span className="ingestion-message">{message}</span>}</div>
    <div className="task-list" style={{ marginTop: 0 }}>
      {requests.length === 0 ? <div className="empty-list">No pending approvals.</div> : requests.map((r) => <div className="task-item" key={r.id}><div style={{ flex: 1 }}><strong style={{ display: 'block' }}>{r.capability}</strong><span style={{ color: 'var(--muted)', fontSize: '10px' }}>{r.action_description} · {new Date(r.requested_at).toLocaleString()}</span></div><button className="primary-button" onClick={() => void approve(r.id)} style={{ marginRight: '6px' }}>✓</button><button className="quiet-button" onClick={() => void deny(r.id)}>✗</button></div>)}
    </div>
  </section>
}

// ── Threads View ───────────────────────────────────────────────────────────

function ThreadsView({ workspace }: { workspace: Workspace }) {
  const [threads, setThreads] = useState<EmailThread[]>([])
  const [summary, setSummary] = useState('')
  const [message, setMessage] = useState('')
  const load = async () => { try { setThreads(await rebuildThreads(workspace.id)) } catch (e) { setMessage(String(e)) } }
  useEffect(() => { void load() }, [workspace.id])
  const summarize = async (thread: EmailThread) => { setMessage('Summarizing thread...'); try { const result = await summarizeEmailThread(thread.thread_id, workspace.id, 'qwen3:14b'); setSummary(result); setMessage('Thread summarized.') } catch (e) { setMessage(String(e)) } }
  return <section className="sources-section">
    <div className="empty-section source-intro"><span className="eyebrow accent">Email threads</span><h2>Conversations,<br /><em>rebuilt.</em></h2><p>Threads are reconstructed by grouping emails with the same cleaned subject line. Click refresh to rebuild.</p><button className="primary-button" onClick={() => void load()} style={{ marginTop: '12px' }}>Refresh threads</button>{message && <span className="ingestion-message">{message}</span>}{summary && <div className="summary-panel" style={{ marginTop: '12px' }}><span className="eyebrow">Thread summary</span><div>{summary}</div></div>}</div>
    <div className="source-panel">
      {threads.length === 0 ? <div className="empty-list">No threads found or email not synced.</div> : threads.map((t) => <div className="source-item" key={t.thread_id}><div><strong>{t.subject}</strong><span>{t.message_count} messages · {t.participants.join(', ')}</span><code>{t.oldest_at ? new Date(t.oldest_at).toLocaleDateString() : '—'} — {t.newest_at ? new Date(t.newest_at).toLocaleDateString() : '—'}</code></div><button className="quiet-button" onClick={() => void summarize(t)} title="Summarize thread">📋</button></div>)}
    </div>
  </section>
}


function SettingRow({ title, description, children }: { title: string; description: string; children: ReactNode }) { return <div className="setting-row"><div><strong>{title}</strong><span>{description}</span></div>{children}</div> }
function Toggle({ checked, onChange }: { checked: boolean; onChange: (value: boolean) => void }) { return <button className={`toggle ${checked ? 'on' : ''}`} onClick={() => onChange(!checked)} aria-pressed={checked}><span /></button> }

// ── 3D Assistant View ──────────────────────────────────────────────────────

function ThreeDView({ workspace }: { workspace: Workspace }) {
  const [state, setState] = useState<'idle' | 'listening' | 'speaking' | 'thinking'>('idle')
  const [prompt, setPrompt] = useState('')
  const [response, setResponse] = useState('')
  const [working, setWorking] = useState(false)

  const ask = async () => {
    if (!prompt.trim()) return
    setState('thinking')
    setWorking(true)
    try {
      const result = await askAssistant(workspace.id, prompt)
      setResponse(result.response)
      setState('speaking')
      if ('speechSynthesis' in window) {
        const utterance = new SpeechSynthesisUtterance(result.response)
        utterance.onend = () => setState('idle')
        window.speechSynthesis.speak(utterance)
      } else {
        setTimeout(() => setState('idle'), 3000)
      }
    } catch (error) {
      setResponse(errMsg(error))
      setState('idle')
    } finally {
      setWorking(false)
    }
  }

  return (
    <section className="three-d-section">
      <div className="three-d-hero">
        <div>
          <span className="eyebrow accent">3D Assistant</span>
          <h2>Your companion,<br /><em>visualized.</em></h2>
          <p>The 3D character responds to assistant state — idle, listening, speaking, and thinking. Click it to toggle voice input, or use the chat below.</p>
        </div>
        <ThreeDAssistant state={state} size={220} onStateChange={(s) => s === 'idle' ? setState('listening') : setState('idle')} />
      </div>
      <div className="ingestion-panel">
        <span className="eyebrow">Chat with the assistant</span>
        <div className="ingestion-form">
          <input value={prompt} onChange={(e) => setPrompt(e.target.value)} placeholder="Ask about your knowledge base..." onKeyDown={(e) => { if (e.key === 'Enter') void ask() }} />
          <button className="primary-button" disabled={working} onClick={() => void ask()}>{working ? 'Thinking...' : 'Ask'}</button>
        </div>
        {response && <div className="assistant-response">{response}</div>}
      </div>
    </section>
  )
}

// ── Cost Limits View ───────────────────────────────────────────────────────

function CostsView({ workspace }: { workspace: Workspace }) {
  const [summary, setSummary] = useState<CostSummary | null>(null)
  const [limit, setLimit] = useState('')
  const [capability, setCapability] = useState('chat')
  const [message, setMessage] = useState('')

  useEffect(() => {
    void getCostSummary(workspace.id).then(setSummary).catch(() => setSummary(null))
  }, [workspace.id])

  const save = async () => {
    if (!limit.trim()) return setMessage('Enter a limit in cents.')
    try {
      await setCostLimit(workspace.id, capability, parseInt(limit))
      setMessage(`${capability} cost limit set to ${limit} cents.`)
    } catch (e) {
      setMessage(e instanceof Error ? e.message : 'Failed to set limit.')
    }
  }

  return (
    <section className="settings-section">
      <div className="settings-intro">
        <span className="eyebrow accent">Cost controls</span>
        <h2>Keep track of<br /><em>cloud spending.</em></h2>
        <p>Set per-capability cost limits and monitor token usage across providers.</p>
      </div>
      <div className="settings-panel">
        <div className="setting-row">
          <div><strong>Total tokens used</strong><span>Across all providers in this workspace</span></div>
          <span style={{ fontFamily: 'DM Mono', color: 'var(--accent)', fontSize: '16px' }}>{summary?.total_tokens?.toLocaleString() ?? '—'}</span>
        </div>
        <div className="setting-row">
          <div><strong>Total cost</strong><span>In USD</span></div>
          <span style={{ fontFamily: 'DM Mono', color: 'var(--green)', fontSize: '16px' }}>${((summary?.total_cost_cents ?? 0) / 100).toFixed(3)}</span>
        </div>
        {summary?.by_provider?.map((p) => (
          <div className="setting-row" key={p.provider}>
            <div><strong>{p.provider}</strong><span>{p.tokens.toLocaleString()} tokens</span></div>
            <span className="pill pill-cloud">${(p.cost_cents / 100).toFixed(3)}</span>
          </div>
        ))}
        <div className="setting-row">
          <div><strong>Set cost limit</strong><span>Capability will switch to local when exceeded</span></div>
          <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
            <select value={capability} onChange={(e) => setCapability(e.target.value)} style={{ border: '1px solid var(--line)', borderRadius: '5px', padding: '8px', color: 'var(--soft)', background: '#10131a', fontSize: '10px' }}>
              <option value="chat">Chat</option>
              <option value="transcription">Transcription</option>
              <option value="embeddings">Embeddings</option>
              <option value="speech">Speech</option>
            </select>
            <input value={limit} onChange={(e) => setLimit(e.target.value)} placeholder="Cents" style={{ width: '80px', border: '1px solid var(--line)', borderRadius: '5px', padding: '8px', color: 'var(--soft)', background: '#10131a', fontSize: '10px' }} />
            <button className="primary-button" onClick={() => void save()}>Save</button>
          </div>
        </div>
        {message && <span className="ingestion-message">{message}</span>}
      </div>
    </section>
  )
}
