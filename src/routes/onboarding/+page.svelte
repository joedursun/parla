<script lang="ts">
	import { goto } from '$app/navigation';
	import { createProfile } from '$lib/conversation';
	import { setUserProfile } from '$lib/stores.svelte';

	let step = $state(1);
	let saving = $state(false);

	const languages = [
		{ flag: '\u{1F1EA}\u{1F1F8}', name: 'Spanish', native: 'Espanol' },
		{ flag: '\u{1F1EB}\u{1F1F7}', name: 'French', native: 'Francais' },
		{ flag: '\u{1F1EF}\u{1F1F5}', name: 'Japanese', native: '\u65E5\u672C\u8A9E' },
		{ flag: '\u{1F1E9}\u{1F1EA}', name: 'German', native: 'Deutsch' },
		{ flag: '\u{1F1EE}\u{1F1F9}', name: 'Italian', native: 'Italiano' },
		{ flag: '\u{1F1F0}\u{1F1F7}', name: 'Korean', native: '\uD55C\uAD6D\uC5B4' },
		{ flag: '\u{1F1F5}\u{1F1F9}', name: 'Portuguese', native: 'Portugues' },
		{ flag: '\u{1F1E8}\u{1F1F3}', name: 'Mandarin', native: '\u4E2D\u6587' },
	];

	const levels = [
		{ code: 'A1', label: 'Complete Beginner', desc: "I've never studied this language, or I only know a few words", style: 'beginner' },
		{ code: 'A2', label: 'Elementary', desc: 'I can handle simple conversations about familiar topics', style: 'elementary' },
		{ code: 'B1', label: 'Intermediate', desc: 'I can discuss everyday topics and understand main points', style: 'intermediate' },
		{ code: 'B2+', label: 'Advanced', desc: 'I can hold detailed conversations and read most texts', style: 'advanced' },
	];

	const goals = [
		{ icon: '\u{1F30D}', title: 'Travel', desc: 'Navigate, order food, ask for directions' },
		{ icon: '\u{1F4BC}', title: 'Work & Business', desc: 'Professional vocabulary, meetings, emails' },
		{ icon: '\u{1F4AC}', title: 'Conversation', desc: 'Chat naturally with native speakers' },
		{ icon: '\u{1F4DA}', title: 'Academic', desc: 'Exams, certifications, formal writing' },
		{ icon: '\u{1F3AC}', title: 'Media & Culture', desc: 'Understand movies, music, books, and news' },
	];

	let selectedLanguage = $state(0);
	let selectedLevel = $state(0);
	let selectedGoals = $state(new Set([0, 2]));

	function toggleGoal(i: number) {
		const next = new Set(selectedGoals);
		if (next.has(i)) next.delete(i);
		else next.add(i);
		selectedGoals = next;
	}

	async function finishOnboarding() {
		if (saving) return;
		saving = true;
		try {
			const lang = languages[selectedLanguage];
			const level = levels[selectedLevel];
			const goalNames = [...selectedGoals].map((i) => goals[i].title);
			const profile = await createProfile('English', lang.name, level.code, goalNames);
			setUserProfile({
				name: 'Learner',
				nativeLanguage: profile.native_language,
				targetLanguage: profile.target_language,
				level: profile.cefr_level,
				levelLabel: level.label,
				goals: profile.goals,
			});
			goto('/');
		} catch (e) {
			console.error('Failed to save profile:', e);
			saving = false;
		}
	}
</script>

<div class="onboarding">
	<div class="onboarding-logo">
		<div class="logo-icon">P</div>
		Parla
	</div>

	<div class="onboarding-card">
		<div class="steps">
			{#each [1, 2, 3, 4] as s}
				<div class="step-dot" class:active={s === step} class:done={s < step}></div>
			{/each}
		</div>

		{#if step === 1}
			<h1>What would you like to learn?</h1>
			<p class="subtitle">Choose the language you want to practice</p>
			<div class="language-grid">
				{#each languages as lang, i}
					<button class="language-option" class:selected={selectedLanguage === i} onclick={() => selectedLanguage = i}>
						<span class="flag">{lang.flag}</span>
						<div>
							<div class="lang-name">{lang.name}</div>
							<div class="lang-native">{lang.native}</div>
						</div>
					</button>
				{/each}
			</div>
			<div class="onboarding-actions">
				<button class="btn btn-primary btn-lg full" onclick={() => step = 2}>Continue</button>
			</div>

		{:else if step === 2}
			<h1>What's your level?</h1>
			<p class="subtitle">This helps your tutor adapt to you right away</p>
			<div class="level-options">
				{#each levels as level, i}
					<button class="level-option" class:selected={selectedLevel === i} onclick={() => selectedLevel = i}>
						<div class="level-badge {level.style}">{level.code}</div>
						<div class="level-info">
							<h4>{level.label}</h4>
							<p>{level.desc}</p>
						</div>
					</button>
				{/each}
			</div>
			<div class="onboarding-actions">
				<button class="btn btn-secondary btn-lg" onclick={() => step = 1}>Back</button>
				<button class="btn btn-primary btn-lg" onclick={() => step = 3}>Continue</button>
			</div>

		{:else if step === 3}
			<h1>What's your goal?</h1>
			<p class="subtitle">Select all that apply — this shapes your lessons</p>
			<div class="goal-options">
				{#each goals as goal, i}
					<button class="goal-option" class:selected={selectedGoals.has(i)} onclick={() => toggleGoal(i)}>
						<span class="goal-icon">{goal.icon}</span>
						<div class="goal-info">
							<h4>{goal.title}</h4>
							<p>{goal.desc}</p>
						</div>
					</button>
				{/each}
			</div>
			<div class="onboarding-actions">
				<button class="btn btn-secondary btn-lg" onclick={() => step = 2}>Back</button>
				<button class="btn btn-primary btn-lg" onclick={() => step = 4}>Continue</button>
			</div>

		{:else}
			<div class="welcome-teacher">
				<div class="teacher-avatar">&#x1F393;</div>
				<h1>Meet your tutor</h1>
				<p class="teacher-greeting">
					I'm your personal {languages[selectedLanguage].name} tutor. I'll adapt to your pace,
					correct your mistakes gently, and make sure every conversation
					moves you forward. Ready to start?
				</p>
				<p class="privacy-note">
					Powered by a local AI running on your machine &mdash;
					your conversations stay private.
				</p>
			</div>
			<button
				class="btn btn-primary btn-lg full start-btn"
				onclick={finishOnboarding}
				disabled={saving}
			>
				{saving ? 'Setting up...' : 'Start my first lesson \u2192'}
			</button>
		{/if}
	</div>
</div>

<style>
	.onboarding {
		max-width: 520px;
		width: 100%;
		padding: var(--space-xl);
		margin: 0 auto;
		min-height: 100vh;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
	}

	.onboarding-logo {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		font-weight: 700;
		font-size: 1.5rem;
		color: var(--primary);
		margin-bottom: var(--space-2xl);
		justify-content: center;
	}

	.logo-icon {
		width: 44px;
		height: 44px;
		background: var(--primary);
		border-radius: var(--radius-lg);
		display: flex;
		align-items: center;
		justify-content: center;
		color: white;
		font-size: 1.25rem;
	}

	.onboarding-card {
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: var(--radius-xl);
		padding: var(--space-2xl);
		box-shadow: var(--shadow-lg);
		width: 100%;
	}

	.onboarding-card h1 { text-align: center; margin-bottom: var(--space-xs); }
	.subtitle { text-align: center; color: var(--text-secondary); margin-bottom: var(--space-xl); font-size: 0.9375rem; }

	.steps { display: flex; align-items: center; justify-content: center; gap: var(--space-sm); margin-bottom: var(--space-xl); }
	.step-dot { width: 8px; height: 8px; border-radius: var(--radius-full); background: var(--border); transition: all var(--transition); }
	.step-dot.active { background: var(--primary); width: 24px; }
	.step-dot.done { background: var(--success); }

	.language-grid { display: grid; grid-template-columns: 1fr 1fr; gap: var(--space-sm); margin-bottom: var(--space-xl); }
	.language-option { display: flex; align-items: center; gap: var(--space-md); padding: var(--space-md); border: 2px solid var(--border); border-radius: var(--radius-lg); cursor: pointer; transition: all var(--transition); background: var(--surface); text-align: left; }
	.language-option:hover { border-color: var(--primary-light); background: var(--primary-subtle); }
	.language-option.selected { border-color: var(--primary); background: var(--primary-subtle); }
	.flag { font-size: 1.75rem; line-height: 1; }
	.lang-name { font-weight: 600; font-size: 0.9375rem; }
	.lang-native { font-size: 0.8125rem; color: var(--text-muted); }

	.level-options { display: flex; flex-direction: column; gap: var(--space-sm); margin-bottom: var(--space-xl); }
	.level-option { display: flex; align-items: flex-start; gap: var(--space-md); padding: var(--space-md) var(--space-lg); border: 2px solid var(--border); border-radius: var(--radius-lg); cursor: pointer; transition: all var(--transition); background: var(--surface); text-align: left; }
	.level-option:hover { border-color: var(--primary-light); background: var(--primary-subtle); }
	.level-option.selected { border-color: var(--primary); background: var(--primary-subtle); }
	.level-badge { width: 40px; height: 40px; border-radius: var(--radius-md); display: flex; align-items: center; justify-content: center; font-weight: 700; font-size: 0.8125rem; flex-shrink: 0; }
	.level-badge.beginner { background: var(--success-subtle); color: var(--success); }
	.level-badge.elementary { background: var(--accent-gold-subtle); color: var(--warning); }
	.level-badge.intermediate { background: var(--secondary-subtle); color: var(--secondary); }
	.level-badge.advanced { background: var(--primary-subtle); color: var(--primary); }
	.level-info h4 { margin-bottom: 2px; }
	.level-info p { font-size: 0.8125rem; color: var(--text-secondary); line-height: 1.5; }

	.goal-options { display: flex; flex-direction: column; gap: var(--space-sm); margin-bottom: var(--space-xl); }
	.goal-option { display: flex; align-items: center; gap: var(--space-md); padding: var(--space-md) var(--space-lg); border: 2px solid var(--border); border-radius: var(--radius-lg); cursor: pointer; transition: all var(--transition); background: var(--surface); text-align: left; }
	.goal-option:hover { border-color: var(--primary-light); }
	.goal-option.selected { border-color: var(--primary); background: var(--primary-subtle); }
	.goal-icon { font-size: 1.5rem; }
	.goal-info h4 { margin-bottom: 2px; }
	.goal-info p { font-size: 0.8125rem; color: var(--text-secondary); }

	.onboarding-actions { display: flex; gap: var(--space-md); justify-content: space-between; }
	.onboarding-actions .btn { flex: 1; }

	.welcome-teacher { text-align: center; padding: var(--space-lg) 0; }
	.teacher-avatar { width: 80px; height: 80px; border-radius: var(--radius-full); background: linear-gradient(135deg, var(--primary), var(--primary-light)); margin: 0 auto var(--space-lg); display: flex; align-items: center; justify-content: center; font-size: 2rem; color: white; box-shadow: 0 4px 20px rgba(124, 111, 224, 0.3); }
	.teacher-greeting { font-size: 1.125rem; color: var(--text-secondary); line-height: 1.6; max-width: 380px; margin: 0 auto var(--space-lg); }
	.privacy-note { font-size: 0.875rem; color: var(--text-muted); margin-bottom: var(--space-lg); }

	.full { width: 100%; }
	.start-btn { font-size: 1.125rem; padding: 14px; text-decoration: none; display: flex; }
</style>
