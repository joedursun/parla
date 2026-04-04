<script lang="ts">
	const skills = [
		{ icon: '\u{1F4D6}', name: 'Reading', pct: 72, color: 'success' },
		{ icon: '\u{1F3A7}', name: 'Listening', pct: 58, color: '' },
		{ icon: '\u{270D}\u{FE0F}', name: 'Writing', pct: 65, color: 'accent' },
		{ icon: '\u{1F5E3}\u{FE0F}', name: 'Speaking', pct: 45, color: 'warning' },
	];

	const weakAreas = [
		{ name: 'Verb conjugation (present)', accuracy: '42%' },
		{ name: 'Gender agreement', accuracy: '51%' },
		{ name: 'Question formation', accuracy: '55%' },
		{ name: 'Pronunciation: rr', accuracy: '38%' },
	];

	const vocabCategories = [
		{ name: 'Greetings', mastered: 85, learning: 10, newPct: 5, count: 32 },
		{ name: 'Numbers', mastered: 70, learning: 20, newPct: 10, count: 45 },
		{ name: 'Directions', mastered: 55, learning: 30, newPct: 15, count: 38 },
		{ name: 'Food & Dining', mastered: 25, learning: 35, newPct: 40, count: 52 },
		{ name: 'Daily Objects', mastered: 15, learning: 15, newPct: 70, count: 20 },
	];

	const grammarConcepts = [
		{ name: 'Subject pronouns', desc: 'yo, tu, el/ella, nosotros...', status: 'mastered' as const },
		{ name: 'Ser vs Estar (basics)', desc: 'Permanent vs temporary states', status: 'mastered' as const },
		{ name: 'Definite & indefinite articles', desc: 'el/la/los/las, un/una/unos/unas', status: 'mastered' as const },
		{ name: 'Present tense conjugation', desc: 'Regular -ar, -er, -ir verbs', status: 'learning' as const },
		{ name: 'Gender & number agreement', desc: 'Adjective-noun agreement', status: 'learning' as const },
		{ name: 'Question words', desc: 'Que, cual, donde, cuando, como', status: 'learning' as const },
		{ name: 'Gustar & similar verbs', desc: 'Verbs that work "backwards"', status: 'upcoming' as const },
		{ name: 'Possessive adjectives', desc: 'mi, tu, su, nuestro...', status: 'upcoming' as const },
	];
</script>

<div class="page-body">
	<!-- Level Hero -->
	<div class="level-hero">
		<div class="level-ring">
			<svg viewBox="0 0 140 140">
				<circle class="ring-bg" cx="70" cy="70" r="60"/>
				<circle class="ring-fill" cx="70" cy="70" r="60"/>
			</svg>
			<div class="ring-label">
				<span class="ring-level">A1</span>
				<span class="ring-sublabel">Beginner</span>
			</div>
		</div>
		<div class="level-info">
			<h2>65% through A1 &mdash; Beginner</h2>
			<p>You're building a solid foundation! You can greet people, ask basic questions, order food, and navigate simple conversations. Keep going &mdash; A2 is within reach.</p>
			<div class="level-milestones">
				<div class="milestone done">&#x2713; Greetings</div>
				<div class="milestone done">&#x2713; Numbers</div>
				<div class="milestone done">&#x2713; Directions</div>
				<div class="milestone">&#x25CB; Food &amp; Dining</div>
				<div class="milestone">&#x25CB; Shopping</div>
			</div>
		</div>
	</div>

	<!-- Skills -->
	<div class="skills-section">
		<h3>Skills Breakdown</h3>
		<div class="skills-grid">
			{#each skills as skill}
				<div class="skill-card">
					<div class="skill-icon">{skill.icon}</div>
					<div class="skill-name">{skill.name}</div>
					<div class="skill-level">A1 &middot; Beginner</div>
					<div class="progress-bar">
						<div class="fill {skill.color}" style="width: {skill.pct}%"></div>
					</div>
					<div class="skill-pct">{skill.pct}%</div>
				</div>
			{/each}
		</div>
	</div>

	<!-- Weak Areas -->
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

	<!-- Vocab & Grammar -->
	<div class="progress-grid">
		<div class="vocab-breakdown">
			<h3>Vocabulary by Topic</h3>
			<div class="vocab-summary">
				<div class="vocab-stat mastered"><div class="v-num">98</div><div class="v-label">Mastered</div></div>
				<div class="vocab-stat learning"><div class="v-num">54</div><div class="v-label">Learning</div></div>
				<div class="vocab-stat new"><div class="v-num">35</div><div class="v-label">New</div></div>
			</div>
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
	</div>
</div>

<style>
	.page-body { padding: var(--space-xl); max-width: 1100px; margin: 0 auto; }

	.level-hero { background: linear-gradient(135deg, var(--primary), var(--primary-light)); border-radius: var(--radius-xl); padding: var(--space-xl) var(--space-2xl); color: white; display: flex; align-items: center; gap: var(--space-2xl); margin-bottom: var(--space-xl); }
	.level-ring { position: relative; width: 140px; height: 140px; flex-shrink: 0; }
	.level-ring svg { width: 140px; height: 140px; transform: rotate(-90deg); }
	:global(.ring-bg) { fill: none; stroke: rgba(255,255,255,0.2); stroke-width: 8; }
	:global(.ring-fill) { fill: none; stroke: white; stroke-width: 8; stroke-linecap: round; stroke-dasharray: 377; stroke-dashoffset: 132; }
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

	.weak-areas { background: var(--secondary-subtle); border: 1px solid #F0C6BD; border-radius: var(--radius-lg); padding: var(--space-lg); margin-bottom: var(--space-xl); }
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
