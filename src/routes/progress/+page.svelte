<script lang="ts">
	import { store } from '$lib/stores.svelte';

	const userProfile = $derived(store.userProfile);
	const skills = $derived(store.skills);
	const weakAreas = $derived(store.weakAreas);
	const vocabCategories = $derived(store.vocabCategories);
	const grammarConcepts = $derived(store.grammarConcepts);
	const levelProgress = $derived(store.levelProgress);

	const hasData = $derived(
		skills.length > 0 || weakAreas.length > 0 || vocabCategories.length > 0 || grammarConcepts.length > 0 || levelProgress.level !== ''
	);
</script>

{#if !hasData}
	<div class="empty-page">
		<div class="empty-icon">&#x1F4CA;</div>
		<h2>No progress yet</h2>
		<p>Start a conversation with your tutor to begin tracking your skills, vocabulary, and grammar mastery.</p>
		<a class="btn btn-primary" href="/conversation">Start learning</a>
	</div>
{:else}
	<div class="page-body">
		<!-- Level Hero -->
		{#if levelProgress.level}
			<div class="level-hero">
				<div class="level-ring">
					<svg viewBox="0 0 140 140">
						<circle class="ring-bg" cx="70" cy="70" r="60"/>
						<circle class="ring-fill" cx="70" cy="70" r="60" style="stroke-dashoffset: {377 - (377 * levelProgress.pct / 100)}"/>
					</svg>
					<div class="ring-label">
						<span class="ring-level">{levelProgress.level}</span>
						<span class="ring-sublabel">{levelProgress.label}</span>
					</div>
				</div>
				<div class="level-info">
					<h2>{levelProgress.pct}% through {levelProgress.level} &mdash; {levelProgress.label}</h2>
					{#if levelProgress.description}
						<p>{levelProgress.description}</p>
					{/if}
					{#if levelProgress.milestones.length > 0}
						<div class="level-milestones">
							{#each levelProgress.milestones as m}
								<div class="milestone" class:done={m.done}>
									{m.done ? '\u2713' : '\u25CB'} {m.name}
								</div>
							{/each}
						</div>
					{/if}
				</div>
			</div>
		{/if}

		<!-- Skills -->
		{#if skills.length > 0}
			<div class="skills-section">
				<h3>Skills Breakdown</h3>
				<div class="skills-grid">
					{#each skills as skill}
						<div class="skill-card">
							<div class="skill-icon">{skill.icon}</div>
							<div class="skill-name">{skill.name}</div>
							{#if userProfile}
								<div class="skill-level">{userProfile.level} &middot; {userProfile.levelLabel}</div>
							{/if}
							<div class="progress-bar">
								<div class="fill {skill.color}" style="width: {skill.pct}%"></div>
							</div>
							<div class="skill-pct">{skill.pct}%</div>
						</div>
					{/each}
				</div>
			</div>
		{/if}

		<!-- Weak Areas -->
		{#if weakAreas.length > 0}
			<div class="weak-areas">
				<h3>&#x1F4A1; Areas to Focus On</h3>
				<p>These are concepts where your accuracy has been below 60% recently. Extra practice here will accelerate your progress.</p>
				<div class="weak-list">
					{#each weakAreas as area}
						<div class="weak-item">
							<span>{area.name}</span>
							<span class="accuracy">{area.accuracy}</span>
						</div>
					{/each}
				</div>
			</div>
		{/if}

		<!-- Vocab & Grammar -->
		{#if vocabCategories.length > 0 || grammarConcepts.length > 0}
			<div class="progress-grid">
				{#if vocabCategories.length > 0}
					<div class="vocab-breakdown">
						<h3>Vocabulary by Topic</h3>
						{#if levelProgress.totalMastered > 0 || levelProgress.totalLearning > 0 || levelProgress.totalNew > 0}
							<div class="vocab-summary">
								<div class="vocab-stat mastered"><div class="v-num">{levelProgress.totalMastered}</div><div class="v-label">Mastered</div></div>
								<div class="vocab-stat learning"><div class="v-num">{levelProgress.totalLearning}</div><div class="v-label">Learning</div></div>
								<div class="vocab-stat new"><div class="v-num">{levelProgress.totalNew}</div><div class="v-label">New</div></div>
							</div>
						{/if}
						<div class="category-list">
							{#each vocabCategories as cat}
								<div class="category-item">
									<span class="category-name">{cat.name}</span>
									<div class="category-bar">
										<div class="seg-mastered" style="width:{cat.mastered}%"></div>
										<div class="seg-learning" style="width:{cat.learning}%"></div>
										<div class="seg-new" style="width:{cat.newPct}%"></div>
									</div>
									<span class="category-count">{cat.count}</span>
								</div>
							{/each}
						</div>
					</div>
				{/if}

				{#if grammarConcepts.length > 0}
					<div class="grammar-progress">
						<h3>Grammar Concepts</h3>
						{#each grammarConcepts as concept}
							<div class="grammar-item">
								<div class="grammar-status {concept.status}">
									{#if concept.status === 'mastered'}&#x2713;{:else if concept.status === 'learning'}&#x25CB;{:else}&#x2014;{/if}
								</div>
								<div class="grammar-info">
									<div class="grammar-name">{concept.name}</div>
									<div class="grammar-desc">{concept.desc}</div>
								</div>
								<span class="tag" class:tag-success={concept.status === 'mastered'} class:tag-warning={concept.status === 'learning'} class:tag-muted={concept.status === 'upcoming'}>
									{concept.status === 'upcoming' ? 'Upcoming' : concept.status === 'mastered' ? 'Mastered' : 'Learning'}
								</span>
							</div>
						{/each}
					</div>
				{/if}
			</div>
		{/if}
	</div>
{/if}

<style>
	.empty-page { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: var(--space-md); color: var(--text-secondary); text-align: center; padding: var(--space-2xl); }
	.empty-icon { font-size: 3rem; margin-bottom: var(--space-sm); }
	.empty-page h2 { color: var(--text); }
	.empty-page p { max-width: 400px; font-size: 0.9375rem; line-height: 1.6; }

	.page-body { padding: var(--space-xl); max-width: 1100px; margin: 0 auto; }

	.level-hero { background: linear-gradient(135deg, var(--primary), var(--primary-light)); border-radius: var(--radius-xl); padding: var(--space-xl) var(--space-2xl); color: white; display: flex; align-items: center; gap: var(--space-2xl); margin-bottom: var(--space-xl); }
	.level-ring { position: relative; width: 140px; height: 140px; flex-shrink: 0; }
	.level-ring svg { width: 140px; height: 140px; transform: rotate(-90deg); }
	:global(.ring-bg) { fill: none; stroke: rgba(255,255,255,0.2); stroke-width: 8; }
	:global(.ring-fill) { fill: none; stroke: white; stroke-width: 8; stroke-linecap: round; stroke-dasharray: 377; }
	.ring-label { position: absolute; inset: 0; display: flex; flex-direction: column; align-items: center; justify-content: center; }
	.ring-level { font-size: 2rem; font-weight: 800; letter-spacing: -0.02em; }
	.ring-sublabel { font-size: 0.75rem; opacity: 0.8; }
	.level-info h2 { font-size: 1.5rem; margin-bottom: var(--space-xs); }
	.level-info p { opacity: 0.85; font-size: 0.9375rem; margin-bottom: var(--space-md); line-height: 1.6; }
	.level-milestones { display: flex; gap: var(--space-md); }
	.milestone { background: rgba(255,255,255,0.15); border-radius: var(--radius-md); padding: var(--space-sm) var(--space-md); font-size: 0.8125rem; }
	.milestone.done { background: rgba(255,255,255,0.25); }

	.skills-section { margin-bottom: var(--space-xl); }
	.skills-section h3 { margin-bottom: var(--space-md); }
	.skills-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: var(--space-md); }
	.skill-card { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: var(--space-lg); text-align: center; }
	.skill-icon { font-size: 1.5rem; margin-bottom: var(--space-sm); }
	.skill-name { font-weight: 600; font-size: 0.9375rem; margin-bottom: var(--space-xs); }
	.skill-level { font-size: 0.8125rem; color: var(--text-muted); margin-bottom: var(--space-md); }
	.skill-pct { font-size: 0.75rem; color: var(--text-muted); text-align: right; margin-top: var(--space-xs); }

	.weak-areas { background: var(--secondary-subtle); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: var(--space-lg); margin-bottom: var(--space-xl); }
	.weak-areas h3 { color: var(--secondary); margin-bottom: var(--space-sm); }
	.weak-areas p { font-size: 0.8125rem; color: var(--text-secondary); margin-bottom: var(--space-md); }
	.weak-list { display: flex; flex-wrap: wrap; gap: var(--space-sm); }
	.weak-item { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius-md); padding: var(--space-sm) var(--space-md); font-size: 0.8125rem; display: flex; align-items: center; gap: var(--space-sm); }
	.accuracy { font-weight: 700; color: var(--secondary); }

	.progress-grid { display: grid; grid-template-columns: 1fr 1fr; gap: var(--space-lg); }

	.vocab-breakdown { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: var(--space-lg); }
	.vocab-breakdown h3 { margin-bottom: var(--space-md); }
	.vocab-summary { display: flex; gap: var(--space-md); margin-bottom: var(--space-lg); }
	.vocab-stat { flex: 1; text-align: center; padding: var(--space-md); border-radius: var(--radius-md); background: var(--bg); }
	.v-num { font-size: 1.5rem; font-weight: 700; }
	.v-label { font-size: 0.75rem; color: var(--text-muted); }
	.vocab-stat.mastered .v-num { color: var(--success); }
	.vocab-stat.learning .v-num { color: var(--secondary); }
	.vocab-stat.new .v-num { color: var(--primary); }
	.category-list { display: flex; flex-direction: column; gap: var(--space-md); }
	.category-item { display: flex; align-items: center; gap: var(--space-md); }
	.category-name { width: 120px; font-size: 0.8125rem; font-weight: 500; flex-shrink: 0; }
	.category-bar { flex: 1; height: 20px; background: var(--border-light); border-radius: var(--radius-sm); overflow: hidden; display: flex; }
	.seg-mastered { background: var(--success); }
	.seg-learning { background: var(--accent-gold); }
	.seg-new { background: var(--primary-light); }
	.category-count { width: 40px; text-align: right; font-size: 0.8125rem; font-weight: 600; color: var(--text-secondary); }

	.grammar-progress { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: var(--space-lg); }
	.grammar-progress h3 { margin-bottom: var(--space-md); }
	.grammar-item { display: flex; align-items: center; gap: var(--space-md); padding: var(--space-sm) 0; border-bottom: 1px solid var(--border-light); }
	.grammar-item:last-child { border-bottom: none; }
	.grammar-status { width: 28px; height: 28px; border-radius: var(--radius-full); display: flex; align-items: center; justify-content: center; font-size: 0.8125rem; flex-shrink: 0; }
	.grammar-status.mastered { background: var(--success-subtle); color: var(--success); }
	.grammar-status.learning { background: var(--accent-gold-subtle); color: var(--warning); }
	.grammar-status.upcoming { background: var(--border-light); color: var(--text-muted); }
	.grammar-info { flex: 1; min-width: 0; }
	.grammar-name { font-weight: 600; font-size: 0.875rem; }
	.grammar-desc { font-size: 0.75rem; color: var(--text-muted); }
	.tag-muted { background: var(--border-light); color: var(--text-muted); }
</style>
