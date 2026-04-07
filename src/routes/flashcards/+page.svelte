<script lang="ts">
	import { store, setFlashcards } from '$lib/stores.svelte';
	import { getFlashcards, reviewFlashcard } from '$lib/conversation';
	import { invoke } from '@tauri-apps/api/core';
	import { onMount, onDestroy } from 'svelte';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';

	let unlisteners: UnlistenFn[] = [];

	onMount(async () => {
		try {
			const cards = await getFlashcards();
			setFlashcards(cards);
		} catch (e) {
			console.error('Failed to load flashcards:', e);
		}

		unlisteners.push(
			await listen('flashcards-due-count', async () => {
				try {
					const cards = await getFlashcards();
					setFlashcards(cards);
				} catch {}
			}),
		);
	});

	onDestroy(() => {
		for (const un of unlisteners) un();
	});

	const flashcards = $derived(store.flashcards);

	// ── Mode state ──────────────────────────────────────────────────────
	let mode: 'browse' | 'review' = $state('browse');
	let flipped = $state(false);
	let currentIndex = $state(0);
	let reviewedCount = $state(0);

	// ── Browse filter ───────────────────────────────────────────────────
	type FilterTab = 'All' | 'Due Today' | 'New' | 'Learning' | 'Mature';
	let activeTab: FilterTab = $state('All');

	const filteredCards = $derived(
		activeTab === 'All'
			? flashcards
			: activeTab === 'Due Today'
				? flashcards.filter((c) => c.status === 'Due' || c.status === 'New')
				: flashcards.filter((c) => c.status === activeTab.replace(' ', '')),
	);

	// ── Review derivations ──────────────────────────────────────────────
	const dueCards = $derived(flashcards.filter((c) => c.status === 'Due' || c.status === 'New'));
	const current = $derived(dueCards.length > 0 && currentIndex < dueCards.length ? dueCards[currentIndex] : null);
	const progressPct = $derived(
		dueCards.length > 0 ? Math.round((reviewedCount / dueCards.length) * 100) : 0,
	);

	// ── Browse stats ────────────────────────────────────────────────────
	const newCount = $derived(flashcards.filter((c) => c.status === 'New').length);
	const learningCount = $derived(flashcards.filter((c) => c.status === 'Due' || c.status === 'Learning').length);
	const matureCount = $derived(flashcards.filter((c) => c.status === 'Mature').length);

	const tagClass: Record<string, string> = {
		Mature: 'tag-success',
		Learning: 'tag-secondary',
		Due: 'tag-warning',
		New: 'tag-primary',
	};

	const filterTabs: FilterTab[] = ['All', 'Due Today', 'New', 'Learning', 'Mature'];

	// ── Actions ─────────────────────────────────────────────────────────
	function startReview() {
		currentIndex = 0;
		reviewedCount = 0;
		flipped = false;
		reviewStartTime = Date.now();
		mode = 'review';
	}

	function exitReview() {
		mode = 'browse';
		flipped = false;
	}

	let reviewStartTime = $state(Date.now());

	async function rateCard(rating: string) {
		if (!current) return;
		const responseTime = Date.now() - reviewStartTime;
		try {
			await reviewFlashcard(current.id, rating, responseTime);
		} catch (e) {
			console.error('Failed to review flashcard:', e);
		}
		reviewedCount++;
		flipped = false;
		reviewStartTime = Date.now();
		if (currentIndex + 1 < dueCards.length) {
			currentIndex++;
		} else {
			// Refresh flashcards list after completing review.
			try {
				const cards = await getFlashcards();
				setFlashcards(cards);
			} catch {}
			mode = 'browse';
		}
	}

	function speakWord(text: string, e: MouseEvent) {
		e.stopPropagation();
		invoke('speak_text', { text }).catch((err) => console.error('TTS failed:', err));
	}

	function handleKeydown(e: KeyboardEvent) {
		if (mode !== 'review') return;
		if (e.key === ' ') {
			e.preventDefault();
			flipped = !flipped;
		} else if (flipped) {
			if (e.key === '1') rateCard('again');
			else if (e.key === '2') rateCard('hard');
			else if (e.key === '3') rateCard('good');
			else if (e.key === '4') rateCard('easy');
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

{#if flashcards.length === 0}
	<!-- ── Empty state ──────────────────────────────────────────────── -->
	<div class="empty-page">
		<div class="empty-icon">&#x1F0CF;</div>
		<h2>No flashcards yet</h2>
		<p>
			Start a conversation with your tutor to collect vocabulary. New words will automatically
			become flashcards for review.
		</p>
		<a class="btn btn-primary" href="/conversation">Start a conversation</a>
	</div>
{:else if mode === 'review'}
	<!-- ── Review mode ──────────────────────────────────────────────── -->
	<div class="review-layout">
		<div class="review-topbar">
			<button class="exit-btn" onclick={exitReview}>
				&#x2190; Exit Review
			</button>
			<div class="review-progress">
				<span class="progress-text">{reviewedCount} of {dueCards.length}</span>
				<div class="progress-bar" style="height:6px; flex:1;">
					<div class="fill" style="width: {progressPct}%"></div>
				</div>
				<span class="progress-text">{progressPct}%</span>
			</div>
		</div>

		{#if current}
			<div class="card-stage">
				<div class="flashcard-container">
					<!-- svelte-ignore a11y_click_events_have_key_events -->
					<!-- svelte-ignore a11y_no_static_element_interactions -->
					<div class="flashcard" class:flipped onclick={() => (flipped = !flipped)}>
						<div class="card-face card-front">
							<div class="card-type-badge">
								<span class="tag tag-primary">Vocabulary</span>
							</div>
							<div class="prompt-label">What does this mean?</div>
							<div class="prompt-word">{current.word}</div>
							{#if current.exampleTarget}
								<div class="prompt-context">"{current.exampleTarget}"</div>
							{/if}
							<button class="prompt-audio" onclick={(e) => speakWord(current.word, e)}>&#x1F50A;</button>
							<div class="flip-hint">Click or press Space to reveal</div>
						</div>
						<div class="card-face card-back">
							<div class="card-type-badge">
								<span class="tag tag-primary">Vocabulary</span>
							</div>
							<div class="answer-word">{current.word}</div>
							{#if current.pronunciation}
								<div class="answer-pronunciation">{current.pronunciation}</div>
							{/if}
							<div class="answer-meaning">{current.meaning}</div>
							{#if current.exampleTarget}
								<div class="example-sentence">
									<div class="ex-target">{current.exampleTarget}</div>
									<div class="ex-native">{current.exampleNative}</div>
								</div>
							{/if}
						</div>
					</div>
				</div>
			</div>

			<div class="rating-area" class:visible={flipped}>
				<div class="rating-label">How well did you know this?</div>
				<div class="rating-buttons">
					<button class="rate-btn again" onclick={() => rateCard('again')}
						><span class="rate-label">Again</span><span class="rate-interval"
							>&lt; 1 min</span
						></button
					>
					<button class="rate-btn hard" onclick={() => rateCard('hard')}
						><span class="rate-label">Hard</span><span class="rate-interval"
							>6 min</span
						></button
					>
					<button class="rate-btn good" onclick={() => rateCard('good')}
						><span class="rate-label">Good</span><span class="rate-interval"
							>3 days</span
						></button
					>
					<button class="rate-btn easy" onclick={() => rateCard('easy')}
						><span class="rate-label">Easy</span><span class="rate-interval"
							>7 days</span
						></button
					>
				</div>
				<div class="rating-shortcut">1 = Again, 2 = Hard, 3 = Good, 4 = Easy</div>
			</div>
		{/if}
	</div>
{:else}
	<!-- ── Browse mode ──────────────────────────────────────────────── -->
	<div class="browse-layout">
		<div class="browse-hero">
			<div class="hero-stats">
				<div class="stat-pill new">
					<span class="stat-num">{newCount}</span>
					<span class="stat-label">New</span>
				</div>
				<div class="stat-pill learning">
					<span class="stat-num">{learningCount}</span>
					<span class="stat-label">Learning</span>
				</div>
				<div class="stat-pill mature">
					<span class="stat-num">{matureCount}</span>
					<span class="stat-label">Mature</span>
				</div>
			</div>
			{#if dueCards.length > 0}
				<button class="btn btn-primary review-cta" onclick={startReview}>
					Review {dueCards.length} card{dueCards.length === 1 ? '' : 's'}
				</button>
			{:else}
				<div class="all-caught-up">All caught up! No cards due right now.</div>
			{/if}
		</div>

		<div class="browse-section">
			<div class="browse-header">
				<h3>Your Cards</h3>
				<div class="browse-controls">
					<div class="browse-tabs">
						{#each filterTabs as tab}
							<button
								class="browse-tab"
								class:active={activeTab === tab}
								onclick={() => (activeTab = tab)}
							>
								{tab}
							</button>
						{/each}
					</div>
				</div>
			</div>
			<div class="card-list">
				{#each filteredCards as card}
					<div class="card-preview">
						<div class="card-preview-header">
							<span class="preview-word">{card.word}</span>
							<span
								class="tag {tagClass[card.status] ?? 'tag-primary'}"
								style="font-size:0.6875rem;">{card.status}</span
							>
						</div>
						<div class="preview-meaning">{card.meaning}</div>
						<div class="card-preview-footer">
							<span
								>{card.status === 'New'
									? 'Not yet studied'
									: `Next review: ${card.nextReview}`}</span
							>
							<div class="srs-indicator">
								{#each card.dots as filled}
									<div class="srs-dot" class:filled></div>
								{/each}
							</div>
						</div>
					</div>
				{/each}
				{#if filteredCards.length === 0}
					<div class="no-filter-results">No cards match this filter.</div>
				{/if}
			</div>
		</div>
	</div>
{/if}

<style>
	/* ── Empty state ──────────────────────────────────────────────────── */
	.empty-page {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: var(--space-md);
		color: var(--text-secondary);
		text-align: center;
		padding: var(--space-2xl);
	}
	.empty-icon {
		font-size: 3rem;
		margin-bottom: var(--space-sm);
	}
	.empty-page h2 {
		color: var(--text);
	}
	.empty-page p {
		max-width: 400px;
		font-size: 0.9375rem;
		line-height: 1.6;
	}

	/* ── Review mode ──────────────────────────────────────────────────── */
	.review-layout {
		flex: 1;
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.review-topbar {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		padding: var(--space-md) var(--space-xl);
		border-bottom: 1px solid var(--border);
		background: var(--surface);
	}
	.exit-btn {
		background: none;
		border: none;
		color: var(--text-secondary);
		font-size: 0.875rem;
		font-weight: 600;
		cursor: pointer;
		padding: var(--space-xs) var(--space-sm);
		border-radius: var(--radius-md);
		transition: all var(--transition);
		white-space: nowrap;
	}
	.exit-btn:hover {
		color: var(--text);
		background: var(--bg);
	}
	.review-progress {
		flex: 1;
		display: flex;
		align-items: center;
		gap: var(--space-sm);
	}
	.progress-text {
		font-size: 0.8125rem;
		color: var(--text-muted);
		white-space: nowrap;
	}

	.card-stage {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: var(--space-xl);
	}
	.flashcard-container {
		width: 100%;
		max-width: 560px;
		perspective: 1200px;
	}
	.flashcard {
		width: 100%;
		min-height: 340px;
		position: relative;
		cursor: pointer;
		transform-style: preserve-3d;
		transition: transform 500ms ease;
	}
	.flashcard.flipped {
		transform: rotateY(180deg);
	}
	.card-face {
		position: absolute;
		width: 100%;
		min-height: 340px;
		backface-visibility: hidden;
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: var(--radius-xl);
		box-shadow: var(--shadow-lg);
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: var(--space-2xl);
	}
	.card-back {
		transform: rotateY(180deg);
	}
	.card-type-badge {
		position: absolute;
		top: var(--space-lg);
		left: var(--space-lg);
	}

	.prompt-label {
		font-size: 0.8125rem;
		color: var(--text-muted);
		margin-bottom: var(--space-md);
		text-transform: uppercase;
		letter-spacing: 0.06em;
		font-weight: 600;
	}
	.prompt-word {
		font-size: 2rem;
		font-weight: 700;
		text-align: center;
		margin-bottom: var(--space-sm);
	}
	.prompt-context {
		font-size: 0.9375rem;
		color: var(--text-secondary);
		text-align: center;
		font-style: italic;
	}
	.prompt-audio {
		margin-top: var(--space-lg);
		width: 48px;
		height: 48px;
		border-radius: var(--radius-full);
		background: var(--primary-subtle);
		color: var(--primary);
		border: none;
		font-size: 1.25rem;
		cursor: pointer;
		transition: all var(--transition);
	}
	.prompt-audio:hover {
		background: var(--primary);
		color: white;
		transform: scale(1.1);
	}
	.flip-hint {
		position: absolute;
		bottom: var(--space-lg);
		font-size: 0.8125rem;
		color: var(--text-muted);
	}

	.answer-word {
		font-size: 1.75rem;
		font-weight: 700;
		text-align: center;
		color: var(--primary);
		margin-bottom: var(--space-xs);
	}
	.answer-pronunciation {
		font-size: 0.9375rem;
		color: var(--text-muted);
		margin-bottom: var(--space-md);
	}
	.answer-meaning {
		font-size: 1.125rem;
		text-align: center;
		margin-bottom: var(--space-lg);
	}
	.example-sentence {
		background: var(--bg);
		border-radius: var(--radius-md);
		padding: var(--space-md);
		width: 100%;
		text-align: center;
	}
	.ex-target {
		font-weight: 600;
		font-size: 0.9375rem;
		margin-bottom: 4px;
	}
	.ex-native {
		font-size: 0.8125rem;
		color: var(--text-secondary);
		font-style: italic;
	}

	.rating-area {
		padding: var(--space-lg) var(--space-xl) var(--space-xl);
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-md);
		opacity: 0;
		pointer-events: none;
		transition: opacity 200ms ease;
	}
	.rating-area.visible {
		opacity: 1;
		pointer-events: auto;
	}
	.rating-label {
		font-size: 0.875rem;
		color: var(--text-secondary);
		font-weight: 500;
	}
	.rating-buttons {
		display: flex;
		gap: var(--space-sm);
	}
	.rate-btn {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 4px;
		padding: var(--space-md) var(--space-xl);
		border-radius: var(--radius-lg);
		border: 2px solid var(--border);
		background: var(--surface);
		cursor: pointer;
		transition: all var(--transition);
		min-width: 100px;
	}
	.rate-btn:hover {
		transform: translateY(-2px);
		box-shadow: var(--shadow-md);
	}
	.rate-label {
		font-weight: 700;
		font-size: 0.9375rem;
	}
	.rate-interval {
		font-size: 0.75rem;
		color: var(--text-muted);
	}
	.rate-btn.again {
		border-color: rgba(224, 85, 85, 0.35);
	}
	.rate-btn.again:hover {
		background: rgba(224, 85, 85, 0.1);
		border-color: var(--danger);
	}
	.rate-btn.again .rate-label {
		color: var(--danger);
	}
	.rate-btn.hard {
		border-color: rgba(232, 168, 37, 0.35);
	}
	.rate-btn.hard:hover {
		background: rgba(232, 168, 37, 0.1);
		border-color: var(--warning);
	}
	.rate-btn.hard .rate-label {
		color: var(--warning);
	}
	.rate-btn.good {
		border-color: rgba(93, 190, 138, 0.35);
	}
	.rate-btn.good:hover {
		background: rgba(93, 190, 138, 0.1);
		border-color: var(--success);
	}
	.rate-btn.good .rate-label {
		color: var(--success);
	}
	.rate-btn.easy {
		border-color: rgba(124, 111, 224, 0.35);
	}
	.rate-btn.easy:hover {
		background: rgba(124, 111, 224, 0.1);
		border-color: var(--primary);
	}
	.rate-btn.easy .rate-label {
		color: var(--primary);
	}
	.rating-shortcut {
		font-size: 0.75rem;
		color: var(--text-muted);
	}

	/* ── Browse mode ──────────────────────────────────────────────────── */
	.browse-layout {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow-y: auto;
	}

	.browse-hero {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: var(--space-xl) var(--space-xl) var(--space-lg);
		border-bottom: 1px solid var(--border);
		background: var(--surface);
	}
	.hero-stats {
		display: flex;
		gap: var(--space-lg);
	}
	.stat-pill {
		display: flex;
		align-items: baseline;
		gap: var(--space-xs);
	}
	.stat-num {
		font-size: 1.5rem;
		font-weight: 700;
	}
	.stat-label {
		font-size: 0.75rem;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		color: var(--text-muted);
		font-weight: 600;
	}
	.stat-pill.new .stat-num {
		color: var(--primary);
	}
	.stat-pill.learning .stat-num {
		color: var(--warning);
	}
	.stat-pill.mature .stat-num {
		color: var(--success);
	}

	.review-cta {
		font-size: 0.9375rem;
		padding: var(--space-sm) var(--space-xl);
	}
	.all-caught-up {
		font-size: 0.875rem;
		color: var(--text-muted);
		font-style: italic;
	}

	.browse-section {
		flex: 1;
	}
	.browse-header {
		padding: var(--space-lg) var(--space-xl);
		display: flex;
		align-items: center;
		justify-content: space-between;
	}
	.browse-header h3 {
		margin: 0;
	}
	.browse-controls {
		display: flex;
		gap: var(--space-md);
		align-items: center;
	}
	.browse-tabs {
		display: flex;
		gap: 4px;
		background: var(--bg);
		border-radius: var(--radius-md);
		padding: 3px;
	}
	.browse-tab {
		padding: var(--space-xs) var(--space-md);
		border-radius: var(--radius-sm);
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--text-muted);
		border: none;
		background: none;
		cursor: pointer;
		transition: all var(--transition);
	}
	.browse-tab.active {
		background: var(--surface);
		color: var(--text);
		box-shadow: var(--shadow-sm);
	}

	.card-list {
		padding: 0 var(--space-xl) var(--space-xl);
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
		gap: var(--space-md);
	}
	.card-preview {
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		padding: var(--space-md);
		cursor: pointer;
		transition: all var(--transition);
	}
	.card-preview:hover {
		border-color: var(--primary-light);
		box-shadow: var(--shadow-md);
	}
	.card-preview-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: var(--space-sm);
	}
	.preview-word {
		font-weight: 700;
		font-size: 1rem;
	}
	.preview-meaning {
		font-size: 0.8125rem;
		color: var(--text-secondary);
		margin-bottom: var(--space-sm);
	}
	.card-preview-footer {
		display: flex;
		align-items: center;
		justify-content: space-between;
		font-size: 0.75rem;
		color: var(--text-muted);
	}
	.srs-indicator {
		display: flex;
		gap: 3px;
	}
	.srs-dot {
		width: 8px;
		height: 8px;
		border-radius: var(--radius-full);
		background: var(--border-light);
	}
	.srs-dot.filled {
		background: var(--success);
	}
	.no-filter-results {
		grid-column: 1 / -1;
		text-align: center;
		padding: var(--space-2xl);
		color: var(--text-muted);
		font-size: 0.9375rem;
	}
</style>
