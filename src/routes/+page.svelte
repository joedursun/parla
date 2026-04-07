<script lang="ts">
	import { store } from '$lib/stores.svelte';
	import { startLesson } from '$lib/conversation';
	import { goto } from '$app/navigation';

	const userProfile = $derived(store.userProfile);
	const lessons = $derived(store.lessons);
	const recentVocabulary = $derived(store.recentVocabulary);
	const activityData = $derived(store.activityData);
	const dailyStats = $derived(store.dailyStats);
	const flashcardsDueCount = $derived(store.flashcardsDueCount);
	const currentLesson = $derived(store.currentLesson);

	/** The next lesson the user should do. */
	const nextLesson = $derived(lessons.find((l) => l.status === 'current'));

	async function handleStartLesson(lessonId: number) {
		try {
			const result = await startLesson(lessonId);
			store.currentLesson = {
				title: result.title,
				description: result.description,
				tags: [result.cefrLevel, result.topic],
			};
			goto('/conversation');
		} catch (e) {
			console.error('Failed to start lesson:', e);
		}
	}
</script>

<div class="page-body">
	<div class="greeting">
		{#if userProfile}
			<h1>Welcome back, {userProfile.name}</h1>
			<p>Learning {userProfile.targetLanguage}{#if flashcardsDueCount > 0} &middot; You have {flashcardsDueCount} card{flashcardsDueCount !== 1 ? 's' : ''} to review{/if}</p>
		{:else}
			<h1>Welcome to Parla</h1>
			<p>Set up your profile to get started</p>
		{/if}
	</div>

	<div class="action-grid">
		{#if nextLesson}
			<button class="action-card primary-action" onclick={() => handleStartLesson(nextLesson.id)}>
				<div class="action-icon">&#x1F393;</div>
				<div>
					<div class="action-title">Continue Lesson</div>
					<div class="action-meta">{nextLesson.title}</div>
				</div>
			</button>
		{:else}
			<a class="action-card primary-action" href="/conversation">
				<div class="action-icon">&#x1F393;</div>
				<div>
					<div class="action-title">Start Conversation</div>
					<div class="action-meta">Practice speaking with your tutor</div>
				</div>
			</a>
		{/if}
		<a class="action-card" href="/flashcards">
			<div class="action-icon cards">&#x1F0CF;</div>
			<div>
				<div class="action-title">Review Flashcards</div>
				<div class="action-meta">{flashcardsDueCount > 0 ? `${flashcardsDueCount} card${flashcardsDueCount !== 1 ? 's' : ''} due today` : 'No cards due'}</div>
			</div>
		</a>
		<a class="action-card" href="/conversation">
			<div class="action-icon chat">&#x1F30D;</div>
			<div>
				<div class="action-title">Free Conversation</div>
				<div class="action-meta">Practice speaking about anything</div>
			</div>
		</a>
		<a class="action-card" href="/conversation?autostart=lesson">
			<div class="action-icon lesson">&#x1F9D1;&#x200D;&#x1F3EB;</div>
			<div>
				<div class="action-title">Start Lesson</div>
				<div class="action-meta">Your tutor picks a topic and leads</div>
			</div>
		</a>
	</div>

	{#if dailyStats.streak > 0 || dailyStats.wordsLearned > 0}
		<div class="stat-row">
			<div class="stat-card">
				<div class="stat-icon streak">&#x1F525;</div>
				<div class="stat-value">{dailyStats.streak}</div>
				<div class="stat-label">Day streak</div>
			</div>
			<div class="stat-card">
				<div class="stat-icon vocab">&#x1F4D6;</div>
				<div class="stat-value">{dailyStats.wordsLearned}</div>
				<div class="stat-label">Words learned{#if dailyStats.wordsLearnedTrend} <span class="stat-trend up">{dailyStats.wordsLearnedTrend}</span>{/if}</div>
			</div>
			<div class="stat-card">
				<div class="stat-icon time">&#x23F1;</div>
				<div class="stat-value">{dailyStats.practiceTime}</div>
				<div class="stat-label">Practice this week{#if dailyStats.practiceTimeTrend} <span class="stat-trend up">{dailyStats.practiceTimeTrend}</span>{/if}</div>
			</div>
			<div class="stat-card">
				<div class="stat-icon accuracy">&#x1F3AF;</div>
				<div class="stat-value">{dailyStats.flashcardAccuracy}</div>
				<div class="stat-label">Flashcard accuracy{#if dailyStats.flashcardAccuracyTrend} <span class="stat-trend up">{dailyStats.flashcardAccuracyTrend}</span>{/if}</div>
			</div>
		</div>
	{/if}

	<div class="content-grid">
		<div class="lesson-card">
			<div class="lesson-card-header">
				<h3>Your Learning Path</h3>
				{#if userProfile}
					<span class="tag tag-primary">{userProfile.level} &mdash; {userProfile.levelLabel}</span>
				{/if}
			</div>
			{#if lessons.length > 0}
				{#each lessons as lesson, i}
					<button
						class="lesson-item"
						class:current={lesson.status === 'current'}
						onclick={() => lesson.status === 'current' && handleStartLesson(lesson.id)}
						disabled={lesson.status === 'upcoming'}
					>
						<div class="lesson-num" class:done={lesson.status === 'done'} class:active={lesson.status === 'current'} class:upcoming={lesson.status === 'upcoming'}>
							{#if lesson.status === 'done'}&#x2713;{:else}{i + 1}{/if}
						</div>
						<div class="lesson-info">
							<div class="lesson-title">{lesson.title}</div>
							<div class="lesson-desc">{lesson.description}</div>
						</div>
						<div class="lesson-progress" class:complete={lesson.status === 'done'} style={lesson.status === 'current' ? 'color: var(--primary);' : ''}>
							{lesson.progress}
						</div>
					</button>
				{/each}
			{:else}
				<div class="empty-state">
					<p>Your learning path will appear here once you start your first lesson.</p>
					<a class="btn btn-primary btn-sm" href="/onboarding">Get started</a>
				</div>
			{/if}
		</div>

		<div class="right-col">
			{#if flashcardsDueCount > 0}
				<div class="review-banner">
					<span class="review-icon">&#x1F4CB;</span>
					<div class="review-text">
						<strong>{flashcardsDueCount} card{flashcardsDueCount !== 1 ? 's' : ''} ready for review</strong>
						<p>Keeps your memory sharp</p>
					</div>
					<a class="btn btn-sm btn-primary" href="/flashcards">Review</a>
				</div>
			{/if}

			{#if recentVocabulary.length > 0}
				<div class="words-panel">
					<div class="words-header">
						<h3>Recent Vocabulary</h3>
						<button class="btn btn-ghost btn-sm">See all</button>
					</div>
					{#each recentVocabulary as word}
						<div class="word-item">
							<div>
								<div class="word-target">{word.target}</div>
								<div class="word-native">{word.native}</div>
							</div>
							<div class="word-strength">
								{#each [0, 1, 2, 3] as i}
									<div class="bar" class:filled={i < word.strength && word.strength >= 3} class:medium={i < word.strength && word.strength === 2} class:weak={i < word.strength && word.strength === 1}></div>
								{/each}
							</div>
						</div>
					{/each}
				</div>
			{/if}

			{#if activityData.length > 0}
				<div class="words-panel">
					<div class="words-header">
						<h3>Activity</h3>
						<span style="font-size:0.875rem;color:var(--text-muted);">Last 4 weeks</span>
					</div>
					<div class="activity-grid">
						{#each activityData as level}
							<div class="activity-cell" class:level-1={level === 1} class:level-2={level === 2} class:level-3={level === 3}></div>
						{/each}
					</div>
				</div>
			{/if}
		</div>
	</div>
</div>

<style>
	.page-body { padding: var(--space-xl); max-width: 1100px; margin: 0 auto; }
	.greeting { margin-bottom: var(--space-xl); }
	.greeting h1 { font-size: 1.5rem; margin-bottom: 4px; }
	.greeting p { color: var(--text-secondary); font-size: 0.9375rem; }

	.action-grid { display: grid; grid-template-columns: 1fr 1fr; gap: var(--space-md); margin-bottom: var(--space-xl); }
	.action-card { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: var(--space-lg); cursor: pointer; transition: all var(--transition); display: flex; align-items: flex-start; gap: var(--space-md); text-decoration: none; color: inherit; }
	.action-card:hover { border-color: var(--primary-light); box-shadow: var(--shadow-md); transform: translateY(-1px); }
	.action-card.primary-action { background: linear-gradient(135deg, var(--primary), var(--primary-light)); border-color: transparent; color: white; }
	.action-card.primary-action .action-meta { color: rgba(255,255,255,0.8); }
	.action-icon { width: 48px; height: 48px; border-radius: var(--radius-md); display: flex; align-items: center; justify-content: center; font-size: 1.5rem; flex-shrink: 0; }
	.action-card.primary-action .action-icon { background: rgba(255,255,255,0.2); }
	.action-icon.chat { background: var(--primary-subtle); }
	.action-icon.cards { background: var(--secondary-subtle); }
	.action-icon.lesson { background: var(--accent-gold-subtle); }
	.action-title { font-weight: 650; font-size: 1rem; margin-bottom: 4px; }
	.action-meta { font-size: 0.8125rem; color: var(--text-secondary); }

	.stat-row { display: flex; gap: var(--space-md); margin-bottom: var(--space-xl); }
	.stat-card { flex: 1; background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: var(--space-lg); display: flex; flex-direction: column; gap: var(--space-xs); }
	.stat-icon { width: 40px; height: 40px; border-radius: var(--radius-md); display: flex; align-items: center; justify-content: center; font-size: 1.25rem; margin-bottom: var(--space-xs); }
	.stat-icon.streak { background: var(--accent-gold-subtle); }
	.stat-icon.vocab { background: var(--primary-subtle); }
	.stat-icon.time { background: var(--success-subtle); }
	.stat-icon.accuracy { background: var(--secondary-subtle); }
	.stat-value { font-size: 1.75rem; font-weight: 700; letter-spacing: -0.02em; }
	.stat-label { font-size: 0.8125rem; color: var(--text-muted); }
	.stat-trend { font-size: 0.75rem; font-weight: 600; }
	.stat-trend.up { color: var(--success); }

	.content-grid { display: grid; grid-template-columns: 1fr 380px; gap: var(--space-lg); }

	.lesson-card { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius-lg); overflow: hidden; }
	.lesson-card-header { padding: var(--space-lg); display: flex; align-items: center; justify-content: space-between; }
	.lesson-item { display: flex; align-items: center; gap: var(--space-md); padding: var(--space-md) var(--space-lg); border-top: 1px solid var(--border-light); cursor: pointer; transition: background var(--transition); width: 100%; background: transparent; border-left: none; border-right: none; border-bottom: none; color: inherit; font: inherit; text-align: left; }
	.lesson-item:hover:not(:disabled) { background: var(--surface-hover); }
	.lesson-item:disabled { cursor: default; opacity: 0.7; }
	.lesson-item.current { background: var(--primary-subtle); border-left: 3px solid var(--primary); }
	.lesson-num { width: 32px; height: 32px; border-radius: var(--radius-full); display: flex; align-items: center; justify-content: center; font-weight: 700; font-size: 0.8125rem; flex-shrink: 0; }
	.lesson-num.done { background: var(--success); color: white; }
	.lesson-num.active { background: var(--primary); color: white; }
	.lesson-num.upcoming { background: var(--border-light); color: var(--text-muted); }
	.lesson-info { flex: 1; min-width: 0; }
	.lesson-title { font-weight: 600; font-size: 0.9375rem; }
	.lesson-desc { font-size: 0.8125rem; color: var(--text-secondary); margin-top: 2px; }
	.lesson-progress { width: 60px; text-align: right; font-size: 0.8125rem; font-weight: 600; color: var(--text-muted); }
	.lesson-progress.complete { color: var(--success); }

	.empty-state { padding: var(--space-lg); text-align: center; color: var(--text-muted); }
	.empty-state p { margin-bottom: var(--space-md); font-size: 0.875rem; }

	.right-col { display: flex; flex-direction: column; gap: var(--space-md); }
	.review-banner { background: var(--accent-gold-subtle); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: var(--space-md) var(--space-lg); display: flex; align-items: center; gap: var(--space-md); }
	.review-icon { font-size: 1.25rem; }
	.review-text { flex: 1; }
	.review-text strong { font-weight: 600; }
	.review-text p { font-size: 0.8125rem; color: var(--text-secondary); }

	.words-panel { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius-lg); }
	.words-header { padding: var(--space-lg); display: flex; align-items: center; justify-content: space-between; }
	.word-item { display: flex; align-items: center; justify-content: space-between; padding: var(--space-sm) var(--space-lg); border-top: 1px solid var(--border-light); }
	.word-target { font-weight: 600; font-size: 0.9375rem; }
	.word-native { font-size: 0.8125rem; color: var(--text-secondary); }
	.word-strength { display: flex; gap: 3px; }
	.word-strength .bar { width: 4px; height: 16px; border-radius: 2px; background: var(--border-light); }
	.word-strength .bar.filled { background: var(--success); }
	.word-strength .bar.medium { background: var(--accent-gold); }
	.word-strength .bar.weak { background: var(--secondary); }

	.activity-grid { display: grid; grid-template-columns: repeat(7, 1fr); gap: 3px; padding: 0 var(--space-lg) var(--space-lg); }
	.activity-cell { aspect-ratio: 1; border-radius: 3px; background: var(--border-light); }
	.activity-cell.level-1 { background: rgba(124, 111, 224, 0.25); }
	.activity-cell.level-2 { background: rgba(124, 111, 224, 0.5); }
	.activity-cell.level-3 { background: var(--primary); }
</style>
