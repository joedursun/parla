<script lang="ts">
	import {
		audioStatus,
		startRecording,
		stopRecordingAndTranscribe,
		stopPlayback,
		llmStatus,
		type StopRecordingResult,
		type AudioStatus,
		type LlmStatus,
	} from '$lib/audio';
	import {
		conversationTurn,
		resetConversation,
		cancelGeneration,
		loadConversation,
		type ConversationTurnResult,
		type NewVocabulary,
		type GrammarNote,
		type SuggestedResponse,
	} from '$lib/conversation';
	import { store } from '$lib/stores.svelte';
	import { page } from '$app/state';

	const userProfile = $derived(store.userProfile);
	const currentLesson = $derived(store.currentLesson);
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import { onMount, onDestroy } from 'svelte';

	let isRecording = $state(false);
	let status: AudioStatus | null = $state(null);
	let llm: LlmStatus | null = $state(null);
	let lastRecording: StopRecordingResult | null = $state(null);
	let processingAudio = $state(false);
	let awaitingTutor = $state(false);
	let textInput = $state('');

	// Live conversation messages from actual voice interaction
	let liveMessages: Message[] = $state([]);

	// Context panel state, populated from the most recent LLM response
	let contextVocab: NewVocabulary[] = $state([]);
	let grammarNotes: GrammarNote[] = $state([]);
	let suggestions: SuggestedResponse[] = $state([]);

	// Currently-streaming tutor bubble (partial text as tokens arrive).
	// We render this as a placeholder bubble until `tutor-message-done` fires.
	let streamingSentences: string[] = $state([]);

	// Poll model readiness (models load in background at app startup)
	let pollTimer: ReturnType<typeof setInterval>;
	let unlisteners: UnlistenFn[] = [];

	onMount(async () => {
		pollTimer = setInterval(async () => {
			try {
				status = await audioStatus();
				llm = await llmStatus();
				if (status.stt_ready && status.tts_ready && status.vad_active && llm.loaded) {
					clearInterval(pollTimer);
				}
			} catch {}
		}, 500);

		// Subscribe to streaming events from the Rust LLM pipeline.
		unlisteners.push(
			await listen<string>('tutor-sentence', (e) => {
				streamingSentences = [...streamingSentences, e.payload];
			})
		);
		unlisteners.push(
			await listen<ConversationTurnResult>('tutor-message-done', (e) => {
				applyTutorResponse(e.payload);
				streamingSentences = [];
			})
		);
	});

	onDestroy(() => {
		clearInterval(pollTimer);
		for (const un of unlisteners) un();
	});

	function applyTutorResponse(result: ConversationTurnResult) {
		const parsed = result.parsed;
		const target = result.tutor_target || (parsed?.tutor_message.target_lang ?? '');
		const native = result.tutor_native || (parsed?.tutor_message.native_lang ?? '');

		const msg: Message = {
			role: 'tutor',
			target,
			translation: native,
		};
		if (parsed?.correction) {
			msg.correction = {
				wrong: parsed.correction.original,
				right: parsed.correction.corrected,
				explain: parsed.correction.explanation,
			};
		}
		if (parsed?.new_vocabulary?.length) {
			// Attach the first one inline; remaining go in the context panel.
			const v = parsed.new_vocabulary[0];
			msg.vocab = { word: v.target_text, meaning: v.native_text };
		}

		liveMessages = [...liveMessages, msg];

		// Update context panel
		if (parsed?.new_vocabulary?.length) {
			const seen = new Set(contextVocab.map((v) => v.target_text));
			const newOnes = parsed.new_vocabulary.filter((v) => !seen.has(v.target_text));
			contextVocab = [...contextVocab, ...newOnes];
		}
		if (parsed?.grammar_note) {
			const seen = new Set(grammarNotes.map((g) => g.title));
			if (!seen.has(parsed.grammar_note.title)) {
				grammarNotes = [...grammarNotes, parsed.grammar_note];
			}
		}
		if (parsed?.suggested_responses?.length) {
			suggestions = parsed.suggested_responses;
		}

		awaitingTutor = false;
	}

	async function sendStudentText(text: string, translation: string = '') {
		const trimmed = text.trim();
		if (!trimmed) return;
		liveMessages = [...liveMessages, { role: 'student', target: trimmed, translation }];
		awaitingTutor = true;
		streamingSentences = [];
		try {
			// conversationTurn resolves after the tutor finishes — the intermediate
			// streaming events update streamingSentences for live UI.
			await conversationTurn(trimmed);
		} catch (e) {
			console.error('conversation_turn failed:', e);
			liveMessages = [
				...liveMessages,
				{
					role: 'tutor',
					target: `[Error: ${e}]`,
					translation: '',
				},
			];
			awaitingTutor = false;
		}
	}

	async function onSendText() {
		const t = textInput;
		textInput = '';
		await sendStudentText(t);
	}

	async function onMicDown() {
		try {
			// Barge-in: stop playback and cancel any in-flight generation
			stopPlayback().catch(() => {});
			cancelGeneration().catch(() => {});
			await startRecording();
			isRecording = true;
		} catch (e) {
			console.error('Failed to start recording:', e);
		}
	}

	async function onMicUp() {
		if (!isRecording) return;
		isRecording = false;
		processingAudio = true;
		try {
			const result = await stopRecordingAndTranscribe();
			lastRecording = result;
			if (result.transcription && result.transcription.trim()) {
				await sendStudentText(result.transcription, result.translation || '');
			}
		} catch (e) {
			console.error('Failed to process recording:', e);
		} finally {
			processingAudio = false;
		}
	}

	async function onNewConversation() {
		await resetConversation();
		liveMessages = [];
		contextVocab = [];
		grammarNotes = [];
		suggestions = [];
		streamingSentences = [];
	}

	function suggestionClick(s: SuggestedResponse) {
		sendStudentText(s.target_lang);
	}

	type Message = {
		role: 'tutor' | 'student';
		target: string;
		translation: string;
		correction?: { wrong: string; right: string; explain: string };
		vocab?: { word: string; meaning: string };
	};

	// ── Load conversation by ID from query param ───────────────────
	let loadedConversationId: string | null = $state(null);

	$effect(() => {
		const id = page.url.searchParams.get('id');
		if (id && id !== loadedConversationId) {
			loadedConversationId = id;
			const numId = parseInt(id, 10);
			if (!isNaN(numId)) {
				loadConversation(numId).then((msgs) => {
					liveMessages = msgs.map((m) => ({
						role: m.role === 'student' ? 'student' as const : 'tutor' as const,
						target: m.content,
						translation: m.translation,
					}));
					contextVocab = [];
					grammarNotes = [];
					suggestions = [];
					streamingSentences = [];
					awaitingTutor = false;
				}).catch((e) => {
					console.error('Failed to load conversation:', e);
				});
			}
		}
	});

	let messagesEl: HTMLDivElement | undefined = $state();

	function scrollToBottom() {
		if (messagesEl) {
			messagesEl.scrollTop = messagesEl.scrollHeight;
		}
	}

	$effect(() => {
		// Trigger on any change to messages or streaming content.
		void liveMessages.length;
		void streamingSentences.length;
		void awaitingTutor;
		// Tick to let the DOM update first.
		requestAnimationFrame(scrollToBottom);
	});

	function canSend() {
		return llm?.loaded && !awaitingTutor;
	}
</script>

<div class="conversation-layout">
	<div class="chat-area">
		<div class="chat-header">
			<div class="tutor-avatar-sm">&#x1F393;</div>
			<div class="chat-header-info">
				<div class="chat-header-title">Your {userProfile?.targetLanguage ?? 'Language'} Tutor</div>
				<div class="chat-header-meta">
					<span class="dot"></span>
					{#if currentLesson}
						Active &middot; {currentLesson.title}
					{:else}
						Free Conversation
					{/if}
				</div>
			</div>
			<div class="chat-header-actions">
				<button class="btn btn-ghost btn-icon" title="Toggle translations">Aa</button>
				<button class="btn btn-ghost btn-icon" title="Audio settings">&#x1F50A;</button>
				<button
					class="btn btn-ghost btn-icon"
					title="Start new conversation"
					onclick={onNewConversation}
					aria-label="Start new conversation">&#x21BB;</button>
			</div>
		</div>

		{#if currentLesson}
			<div class="lesson-banner">
				<span>&#x1F37D;</span>
				<span>Lesson: {currentLesson.title}</span>
			</div>
		{/if}

		<div class="messages" bind:this={messagesEl}>
			{#if liveMessages.length === 0 && !awaitingTutor}
				<div class="empty-hint">
					{#if llm?.loaded}
						<p>Tap the mic or type to start a conversation with your {userProfile?.targetLanguage ?? 'language'} tutor.</p>
					{:else}
						<p>Loading language model… this can take a moment on first launch.</p>
					{/if}
				</div>
			{/if}

			{#each liveMessages as msg}
				<div class="message {msg.role}">
					<div class="message-avatar">
						{#if msg.role === 'tutor'}&#x1F393;{:else}{userProfile?.name?.charAt(0)?.toUpperCase() ?? '?'}{/if}
					</div>
					<div class="message-content">
						<div class="bubble">
							<span class="target-text">{msg.target}</span>
							{#if msg.translation}
								<span class="translation">{msg.translation}</span>
							{/if}
						</div>
					</div>
				</div>

				{#if msg.correction}
					<div class="correction">
						<span class="correction-icon">&#x1F4A1;</span>
						<div>
							<div>
								<span class="wrong">{msg.correction.wrong}</span> &rarr;
								<span class="right">{msg.correction.right}</span>
							</div>
							<div class="explain">{msg.correction.explain}</div>
						</div>
					</div>
				{/if}

				{#if msg.vocab}
					<div class="vocab-card-inline">
						<div>
							<div class="vocab-word">{msg.vocab.word}</div>
							<div class="vocab-meaning">{msg.vocab.meaning}</div>
						</div>
						<button class="vocab-add-btn" title="Save to flashcards">+</button>
					</div>
				{/if}
			{/each}

			{#if awaitingTutor}
				<div class="message tutor">
					<div class="message-avatar">&#x1F393;</div>
					<div class="message-content">
						<div class="bubble streaming">
							{#if streamingSentences.length > 0}
								<span class="target-text">{streamingSentences.join(' ')}</span>
							{:else}
								<span class="typing-dots"><span>.</span><span>.</span><span>.</span></span>
							{/if}
						</div>
					</div>
				</div>
			{/if}
		</div>

		<div class="chat-input-area">
			{#if processingAudio}
				<div class="recording-result" style="color: var(--primary);">
					Processing audio...
				</div>
			{:else if lastRecording}
				<div class="recording-result">
					Recorded {(lastRecording.duration_ms / 1000).toFixed(1)}s
					{#if lastRecording.speech_segments.length > 0}
						&middot; {lastRecording.speech_segments.length} speech segment{lastRecording.speech_segments.length !== 1 ? 's' : ''}
					{/if}
				</div>
			{/if}
			<div class="input-row">
				<textarea
					rows="1"
					placeholder="Type in {userProfile?.targetLanguage ?? 'the target language'} (or {userProfile?.nativeLanguage ?? 'your language'} to translate)..."
					bind:value={textInput}
					onkeydown={(e) => {
						if (e.key === 'Enter' && !e.shiftKey) {
							e.preventDefault();
							if (canSend()) onSendText();
						}
					}}
				></textarea>
				<div class="input-actions">
					<!-- svelte-ignore a11y_no_static_element_interactions -->
					<button
						class="voice-btn"
						class:recording={isRecording}
						title={status?.stt_ready ? 'Hold to speak' : 'Loading models...'}
						disabled={!status?.stt_ready || awaitingTutor}
						onpointerdown={onMicDown}
						onpointerup={onMicUp}
						onpointerleave={onMicUp}
						aria-label="Hold to speak"
					>&#x1F3A4;</button>
					<button
						class="send-btn"
						title="Send message"
						disabled={!canSend() || textInput.trim().length === 0}
						onclick={onSendText}
						aria-label="Send message"
					>&#x27A4;</button>
				</div>
			</div>
			<div class="input-hint">
				{#if isRecording}
					<span class="recording-hint">Recording... release to stop</span>
				{:else if processingAudio}
					<span style="color: var(--primary);">Transcribing...</span>
				{:else if awaitingTutor}
					<span style="color: var(--primary);">Tutor is thinking...</span>
				{:else if !llm?.loaded}
					Loading language model...
				{:else if !status?.stt_ready}
					Loading speech recognition...
				{:else}
					Hold the mic button to speak, or type and press Enter
				{/if}
			</div>
		</div>
	</div>

	<div class="context-panel">
		{#if currentLesson}
			<div class="context-section">
				<h4>&#x1F3AF; Lesson Focus</h4>
				<div class="lesson-focus-card">
					<h3>{currentLesson.title}</h3>
					<p>{currentLesson.description}</p>
					{#if currentLesson.tags.length > 0}
						<div class="focus-tags">
							{#each currentLesson.tags as tag, i}
								<span class="tag {i === 0 ? 'tag-primary' : i === 1 ? 'tag-secondary' : 'tag-warning'}">{tag}</span>
							{/each}
						</div>
					{/if}
				</div>
			</div>
		{/if}

		<div class="context-section">
			<h4>&#x1F4D6; Vocabulary This Lesson</h4>
			{#if contextVocab.length === 0}
				<p class="context-empty">New words will appear here as they come up.</p>
			{:else}
				{#each contextVocab as item}
					<div class="context-vocab-item">
						<div>
							<div class="context-vocab-word">{item.target_text}</div>
							<div class="context-vocab-meaning">{item.native_text}</div>
						</div>
						<span class="new-badge">New</span>
					</div>
				{/each}
			{/if}
		</div>

		<div class="context-section">
			<h4>&#x1F4DD; Grammar Notes</h4>
			{#if grammarNotes.length === 0}
				<p class="context-empty">Grammar tips will appear here as the tutor teaches them.</p>
			{:else}
				{#each grammarNotes as g}
					<div class="grammar-note">
						<h4>{g.title}</h4>
						<p>{g.explanation}</p>
					</div>
				{/each}
			{/if}
		</div>

		<div class="context-section">
			<h4>&#x1F4AC; Try Saying</h4>
			{#if suggestions.length === 0}
				<p class="context-empty">Suggested replies will appear here.</p>
			{:else}
				<div class="suggestion-chips">
					{#each suggestions as s}
						<button class="suggestion-chip" onclick={() => suggestionClick(s)}>
							<div class="chip-text">{s.target_lang}</div>
							<div class="chip-hint">{s.native_lang}</div>
						</button>
					{/each}
				</div>
			{/if}
		</div>
	</div>
</div>

<style>
	.conversation-layout { flex: 1; display: flex; overflow: hidden; height: 100%; }

	.chat-area { flex: 1; display: flex; flex-direction: column; min-width: 0; }
	.chat-header { padding: var(--space-md) var(--space-lg); border-bottom: 1px solid var(--border); background: var(--surface); display: flex; align-items: center; gap: var(--space-md); }
	.tutor-avatar-sm { width: 36px; height: 36px; border-radius: var(--radius-full); background: linear-gradient(135deg, var(--primary), var(--primary-light)); color: white; display: flex; align-items: center; justify-content: center; font-size: 1rem; }
	.chat-header-info { flex: 1; }
	.chat-header-title { font-weight: 650; font-size: 1rem; }
	.chat-header-meta { font-size: 0.8125rem; color: var(--text-muted); display: flex; align-items: center; gap: var(--space-sm); }
	.dot { width: 6px; height: 6px; border-radius: var(--radius-full); background: var(--success); }
	.chat-header-actions { display: flex; gap: var(--space-xs); }

	.lesson-banner { background: var(--primary-subtle); padding: var(--space-sm) var(--space-lg); display: flex; align-items: center; gap: var(--space-sm); font-size: 0.8125rem; color: var(--primary); font-weight: 500; border-bottom: 1px solid var(--border); }
	.progress-mini { margin-left: auto; display: flex; align-items: center; gap: var(--space-sm); }
	.text-xs { font-size: 0.75rem; }

	.messages { flex: 1; overflow-y: auto; padding: var(--space-lg); display: flex; flex-direction: column; gap: var(--space-md); }
	.empty-hint { color: var(--text-muted); text-align: center; padding: var(--space-lg); font-size: 0.875rem; }
	.bubble.streaming { opacity: 0.85; }
	.typing-dots { display: inline-flex; gap: 2px; }
	.typing-dots span { animation: blink 1.2s infinite; font-weight: 700; }
	.typing-dots span:nth-child(2) { animation-delay: 0.2s; }
	.typing-dots span:nth-child(3) { animation-delay: 0.4s; }
	@keyframes blink { 0%, 60%, 100% { opacity: 0.25; } 30% { opacity: 1; } }
	.context-empty { font-size: 0.8125rem; color: var(--text-muted); font-style: italic; }

	.message { display: flex; gap: var(--space-sm); max-width: 75%; animation: fadeIn 200ms ease; }
	@keyframes fadeIn { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: translateY(0); } }
	.message.tutor { align-self: flex-start; }
	.message.student { align-self: flex-end; flex-direction: row-reverse; }

	.message-avatar { width: 32px; height: 32px; border-radius: var(--radius-full); display: flex; align-items: center; justify-content: center; font-size: 0.875rem; flex-shrink: 0; align-self: flex-end; }
	.message.tutor .message-avatar { background: linear-gradient(135deg, var(--primary), var(--primary-light)); color: white; }
	.message.student .message-avatar { background: var(--border-light); color: var(--text-secondary); }

	.message-content { display: flex; flex-direction: column; gap: 4px; }

	.bubble { padding: var(--space-md); border-radius: var(--radius-lg); font-size: 0.9375rem; line-height: 1.6; }
	.message.tutor .bubble { background: var(--surface); border: 1px solid var(--border); border-bottom-left-radius: var(--radius-sm); }
	.message.student .bubble { background: var(--primary); color: white; border-bottom-right-radius: var(--radius-sm); }
	.target-text { font-weight: 500; display: block; }
	.translation { display: block; font-size: 0.8125rem; color: var(--text-muted); margin-top: 4px; font-style: italic; }
	.message.student .bubble .translation { color: rgba(255,255,255,0.7); }



	.correction { background: var(--accent-gold-subtle); border: 1px solid var(--border); border-radius: var(--radius-md); padding: var(--space-sm) var(--space-md); font-size: 0.8125rem; display: flex; align-items: flex-start; gap: var(--space-sm); max-width: 75%; align-self: flex-start; margin-left: 40px; }
	.correction-icon { font-size: 1rem; flex-shrink: 0; margin-top: 1px; }
	.correction .wrong { text-decoration: line-through; color: var(--danger); }
	.correction .right { color: var(--success); font-weight: 600; }
	.correction .explain { color: var(--text-secondary); margin-top: 4px; line-height: 1.5; }

	.vocab-card-inline { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: var(--space-md); max-width: 360px; align-self: flex-start; margin-left: 40px; display: flex; align-items: center; gap: var(--space-md); }
	.vocab-word { font-weight: 700; font-size: 1rem; color: var(--primary); }
	.vocab-meaning { font-size: 0.8125rem; color: var(--text-secondary); }
	.vocab-add-btn { margin-left: auto; background: var(--primary-subtle); color: var(--primary); border: none; border-radius: var(--radius-full); width: 28px; height: 28px; display: flex; align-items: center; justify-content: center; cursor: pointer; font-weight: 700; font-size: 1.125rem; }

	.chat-input-area { padding: var(--space-md) var(--space-lg); border-top: 1px solid var(--border); background: var(--surface); }
	.input-row { display: flex; align-items: flex-end; gap: var(--space-sm); background: var(--bg); border: 2px solid var(--border); border-radius: var(--radius-xl); padding: var(--space-sm) var(--space-sm) var(--space-sm) var(--space-md); transition: border-color var(--transition); }
	.input-row:focus-within { border-color: var(--primary); }
	.input-row textarea { flex: 1; border: none; background: none; font-family: var(--font); font-size: 0.9375rem; resize: none; outline: none; color: var(--text); min-height: 24px; max-height: 120px; padding: var(--space-xs) 0; line-height: 1.5; }
	.input-row textarea::placeholder { color: var(--text-muted); }
	.input-actions { display: flex; gap: 4px; align-items: center; }

	.voice-btn { width: 40px; height: 40px; border-radius: var(--radius-full); border: none; background: var(--secondary); color: white; font-size: 1.125rem; cursor: pointer; display: flex; align-items: center; justify-content: center; transition: all var(--transition); }
	.voice-btn:hover { background: var(--secondary-light); transform: scale(1.05); }
	.voice-btn:disabled { opacity: 0.5; cursor: not-allowed; }
	.voice-btn.recording { background: var(--danger); animation: pulse 1.5s infinite; }
	@keyframes pulse { 0%, 100% { box-shadow: 0 0 0 0 rgba(224, 85, 85, 0.4); } 50% { box-shadow: 0 0 0 8px rgba(224, 85, 85, 0); } }
	.send-btn { width: 40px; height: 40px; border-radius: var(--radius-full); border: none; background: var(--primary); color: white; font-size: 1.125rem; cursor: pointer; display: flex; align-items: center; justify-content: center; transition: all var(--transition); }
	.send-btn:hover { background: var(--primary-dark); }
	.input-hint { text-align: center; font-size: 0.75rem; color: var(--text-muted); margin-top: var(--space-xs); }
	.recording-hint { color: var(--danger); font-weight: 600; }
	.recording-result { text-align: center; font-size: 0.8125rem; color: var(--success); padding: var(--space-xs) 0; font-weight: 500; }

	/* Context panel */
	.context-panel { width: 320px; border-left: 1px solid var(--border); background: var(--surface); display: flex; flex-direction: column; flex-shrink: 0; overflow-y: auto; }
	.context-section { padding: var(--space-lg); border-bottom: 1px solid var(--border-light); }
	.context-section h4 { margin-bottom: var(--space-md); display: flex; align-items: center; gap: var(--space-sm); color: var(--text-secondary); font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.06em; }

	.lesson-focus-card { background: var(--primary-subtle); border-radius: var(--radius-md); padding: var(--space-md); }
	.lesson-focus-card h3 { color: var(--primary); font-size: 0.9375rem; margin-bottom: 4px; }
	.lesson-focus-card p { font-size: 0.8125rem; color: var(--text-secondary); }
	.focus-tags { display: flex; flex-wrap: wrap; gap: 4px; margin-top: var(--space-sm); }

	.context-vocab-item { display: flex; align-items: center; justify-content: space-between; padding: var(--space-sm) 0; border-bottom: 1px solid var(--border-light); }
	.context-vocab-item:last-child { border-bottom: none; }
	.context-vocab-word { font-weight: 600; font-size: 0.875rem; }
	.context-vocab-meaning { font-size: 0.8125rem; color: var(--text-muted); }
	.new-badge { font-size: 0.625rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.05em; background: var(--secondary); color: white; padding: 1px 6px; border-radius: var(--radius-full); }

	.grammar-note { background: var(--bg); border-radius: var(--radius-md); padding: var(--space-md); margin-bottom: var(--space-sm); }
	.grammar-note:last-child { margin-bottom: 0; }
	.grammar-note h4 { font-size: 0.875rem; font-weight: 600; margin-bottom: 4px; text-transform: none; letter-spacing: 0; color: var(--text); }
	.grammar-note p { font-size: 0.8125rem; color: var(--text-secondary); line-height: 1.5; }

	.suggestion-chips { display: flex; flex-direction: column; gap: var(--space-xs); }
	.suggestion-chip { background: var(--bg); border: 1px solid var(--border); border-radius: var(--radius-md); padding: var(--space-sm) var(--space-md); font-size: 0.8125rem; cursor: pointer; transition: all var(--transition); text-align: left; }
	.suggestion-chip:hover { border-color: var(--primary); background: var(--primary-subtle); }
	.chip-text { font-weight: 600; color: var(--text); }
	.chip-hint { color: var(--text-muted); font-size: 0.75rem; }
</style>
