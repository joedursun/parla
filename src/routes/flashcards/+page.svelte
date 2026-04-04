<script lang="ts">
	let flipped = $state(false);

	const cards = [
		{ word: 'la cuenta', meaning: 'the bill / the check', pronunciation: '/la \u02C8kwen.ta/', example: { target: '\u00BFMe trae la cuenta, por favor?', native: 'Can you bring me the bill, please?' }, source: 'Ordering at a cafe \u00B7 Lesson 4', status: 'Mature' as const, tag: 'tag-success', nextReview: '5 days', dots: [true, true, true, true] },
		{ word: 'me gustaria', meaning: 'I would like', pronunciation: '/me \u0263us.ta.\u02C8\u027Ei.a/', example: { target: 'Me gustaria un cafe, por favor.', native: 'I would like a coffee, please.' }, source: 'Lesson 4', status: 'Mature' as const, tag: 'tag-success', nextReview: '3 days', dots: [true, true, true, false] },
		{ word: 'cuanto cuesta', meaning: 'how much does it cost', pronunciation: '/\u02C8kwan.to \u02C8kwes.ta/', example: { target: '\u00BFCuanto cuesta el cafe?', native: 'How much does the coffee cost?' }, source: 'Lesson 4', status: 'Learning' as const, tag: 'tag-secondary', nextReview: '10 min', dots: [true, false, false, false] },
		{ word: 'el postre', meaning: 'dessert', pronunciation: '/el \u02C8pos.t\u027Ee/', example: { target: '\u00BFTienen algun postre?', native: 'Do you have any desserts?' }, source: 'Lesson 4', status: 'Due' as const, tag: 'tag-warning', nextReview: 'Due now', dots: [true, true, false, false] },
		{ word: 'tortilla espanola', meaning: 'Spanish omelette', pronunciation: '', example: { target: 'Tenemos tortilla espanola hoy.', native: 'We have Spanish omelette today.' }, source: 'Lesson 4', status: 'New' as const, tag: 'tag-primary', nextReview: 'Not yet studied', dots: [false, false, false, false] },
		{ word: 'la propina', meaning: 'the tip', pronunciation: '/la p\u027Eo.\u02C8pi.na/', example: { target: 'La propina no es obligatoria.', native: 'The tip is not mandatory.' }, source: 'Lesson 4', status: 'New' as const, tag: 'tag-primary', nextReview: 'Not yet studied', dots: [false, false, false, false] },
	];

	const current = cards[0];
</script>

<div class="flashcard-layout">
	<div class="review-header">
		<div class="review-progress">
			<div class="progress-label">
				<span>Card 5 of 12</span>
				<span>42% complete</span>
			</div>
			<div class="progress-bar" style="height:8px;">
				<div class="fill" style="width: 42%"></div>
			</div>
		</div>
		<div class="review-counts">
			<div class="review-count new"><div class="count-num">3</div><div class="count-label">New</div></div>
			<div class="review-count learning"><div class="count-num">4</div><div class="count-label">Learning</div></div>
			<div class="review-count review"><div class="count-num">5</div><div class="count-label">Review</div></div>
		</div>
	</div>

	<div class="card-stage">
		<div class="flashcard-container">
			<!-- svelte-ignore a11y_click_events_have_key_events -->
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div class="flashcard" class:flipped onclick={() => flipped = !flipped}>
				<div class="card-face card-front">
					<div class="card-type-badge"><span class="tag tag-primary">Vocabulary</span></div>
					<div class="prompt-label">What does this mean?</div>
					<div class="prompt-word">{current.word}</div>
					<div class="prompt-context">"{current.example.target}"</div>
					<button class="prompt-audio">&#x1F50A;</button>
					<div class="flip-hint">Click or press Space to reveal</div>
				</div>
				<div class="card-face card-back">
					<div class="card-type-badge"><span class="tag tag-primary">Vocabulary</span></div>
					<div class="answer-word">{current.word}</div>
					<div class="answer-pronunciation">{current.pronunciation}</div>
					<div class="answer-meaning">{current.meaning}</div>
					<div class="example-sentence">
						<div class="ex-target">{current.example.target}</div>
						<div class="ex-native">{current.example.native}</div>
					</div>
					<div class="answer-source">From: {current.source}</div>
				</div>
			</div>
		</div>
	</div>

	<div class="rating-area">
		<div class="rating-label">How well did you know this?</div>
		<div class="rating-buttons">
			<button class="rate-btn again"><span class="rate-label">Again</span><span class="rate-interval">&lt; 1 min</span></button>
			<button class="rate-btn hard"><span class="rate-label">Hard</span><span class="rate-interval">6 min</span></button>
			<button class="rate-btn good"><span class="rate-label">Good</span><span class="rate-interval">3 days</span></button>
			<button class="rate-btn easy"><span class="rate-label">Easy</span><span class="rate-interval">7 days</span></button>
		</div>
		<div class="rating-shortcut">Keyboard: 1 = Again, 2 = Hard, 3 = Good, 4 = Easy</div>
	</div>
</div>

<!-- Browse cards -->
<div class="browse-section">
	<div class="browse-header">
		<h3>Your Cards</h3>
		<div class="browse-controls">
			<div class="browse-tabs">
				<button class="browse-tab active">All</button>
				<button class="browse-tab">Due Today</button>
				<button class="browse-tab">New</button>
				<button class="browse-tab">Learning</button>
				<button class="browse-tab">Mature</button>
			</div>
			<button class="btn btn-secondary btn-sm">+ Add Card</button>
		</div>
	</div>
	<div class="card-list">
		{#each cards as card}
			<div class="card-preview">
				<div class="card-preview-header">
					<span class="preview-word">{card.word}</span>
					<span class="tag {card.tag}" style="font-size:0.6875rem;">{card.status}</span>
				</div>
				<div class="preview-meaning">{card.meaning}</div>
				<div class="card-preview-footer">
					<span>{card.status === 'New' ? 'Not yet studied' : `Next review: ${card.nextReview}`}</span>
					<div class="srs-indicator">
						{#each card.dots as filled}
							<div class="srs-dot" class:filled></div>
						{/each}
					</div>
				</div>
			</div>
		{/each}
	</div>
</div>

<style>
	.flashcard-layout { flex: 1; display: flex; flex-direction: column; overflow: hidden; }

	.review-header { padding: var(--space-md) var(--space-xl); border-bottom: 1px solid var(--border); background: var(--surface); display: flex; align-items: center; gap: var(--space-lg); }
	.review-progress { flex: 1; }
	.progress-label { display: flex; justify-content: space-between; font-size: 0.8125rem; color: var(--text-secondary); margin-bottom: var(--space-xs); }
	.review-counts { display: flex; gap: var(--space-lg); }
	.review-count { text-align: center; }
	.count-num { font-size: 1.25rem; font-weight: 700; }
	.count-label { font-size: 0.6875rem; text-transform: uppercase; letter-spacing: 0.06em; color: var(--text-muted); }
	.review-count.new .count-num { color: var(--primary); }
	.review-count.learning .count-num { color: var(--secondary); }
	.review-count.review .count-num { color: var(--success); }

	.card-stage { flex: 1; display: flex; align-items: center; justify-content: center; padding: var(--space-xl); }
	.flashcard-container { width: 100%; max-width: 560px; perspective: 1200px; }
	.flashcard { width: 100%; min-height: 340px; position: relative; cursor: pointer; transform-style: preserve-3d; transition: transform 500ms ease; }
	.flashcard.flipped { transform: rotateY(180deg); }
	.card-face { position: absolute; width: 100%; min-height: 340px; backface-visibility: hidden; background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius-xl); box-shadow: var(--shadow-lg); display: flex; flex-direction: column; align-items: center; justify-content: center; padding: var(--space-2xl); }
	.card-back { transform: rotateY(180deg); }
	.card-type-badge { position: absolute; top: var(--space-lg); left: var(--space-lg); }

	.prompt-label { font-size: 0.8125rem; color: var(--text-muted); margin-bottom: var(--space-md); text-transform: uppercase; letter-spacing: 0.06em; font-weight: 600; }
	.prompt-word { font-size: 2rem; font-weight: 700; text-align: center; margin-bottom: var(--space-sm); }
	.prompt-context { font-size: 0.9375rem; color: var(--text-secondary); text-align: center; font-style: italic; }
	.prompt-audio { margin-top: var(--space-lg); width: 48px; height: 48px; border-radius: var(--radius-full); background: var(--primary-subtle); color: var(--primary); border: none; font-size: 1.25rem; cursor: pointer; transition: all var(--transition); }
	.prompt-audio:hover { background: var(--primary); color: white; transform: scale(1.1); }
	.flip-hint { position: absolute; bottom: var(--space-lg); font-size: 0.8125rem; color: var(--text-muted); }

	.answer-word { font-size: 1.75rem; font-weight: 700; text-align: center; color: var(--primary); margin-bottom: var(--space-xs); }
	.answer-pronunciation { font-size: 0.9375rem; color: var(--text-muted); margin-bottom: var(--space-md); }
	.answer-meaning { font-size: 1.125rem; text-align: center; margin-bottom: var(--space-lg); }
	.example-sentence { background: var(--bg); border-radius: var(--radius-md); padding: var(--space-md); width: 100%; text-align: center; }
	.ex-target { font-weight: 600; font-size: 0.9375rem; margin-bottom: 4px; }
	.ex-native { font-size: 0.8125rem; color: var(--text-secondary); font-style: italic; }
	.answer-source { margin-top: var(--space-md); font-size: 0.75rem; color: var(--text-muted); }

	.rating-area { padding: var(--space-lg) var(--space-xl) var(--space-xl); display: flex; flex-direction: column; align-items: center; gap: var(--space-md); }
	.rating-label { font-size: 0.875rem; color: var(--text-secondary); font-weight: 500; }
	.rating-buttons { display: flex; gap: var(--space-sm); }
	.rate-btn { display: flex; flex-direction: column; align-items: center; gap: 4px; padding: var(--space-md) var(--space-xl); border-radius: var(--radius-lg); border: 2px solid var(--border); background: var(--surface); cursor: pointer; transition: all var(--transition); min-width: 100px; }
	.rate-btn:hover { transform: translateY(-2px); box-shadow: var(--shadow-md); }
	.rate-label { font-weight: 700; font-size: 0.9375rem; }
	.rate-interval { font-size: 0.75rem; color: var(--text-muted); }
	.rate-btn.again { border-color: #F5C6C6; }
	.rate-btn.again:hover { background: #FDF0F0; border-color: var(--danger); }
	.rate-btn.again .rate-label { color: var(--danger); }
	.rate-btn.hard { border-color: #F5DFC6; }
	.rate-btn.hard:hover { background: #FFF8F0; border-color: var(--warning); }
	.rate-btn.hard .rate-label { color: var(--warning); }
	.rate-btn.good { border-color: #C6E8D4; }
	.rate-btn.good:hover { background: #F0FDF5; border-color: var(--success); }
	.rate-btn.good .rate-label { color: var(--success); }
	.rate-btn.easy { border-color: #C6D4F5; }
	.rate-btn.easy:hover { background: #F0F5FD; border-color: var(--primary); }
	.rate-btn.easy .rate-label { color: var(--primary); }
	.rating-shortcut { font-size: 0.75rem; color: var(--text-muted); }

	.browse-section { border-top: 1px solid var(--border); background: var(--surface); }
	.browse-header { padding: var(--space-lg) var(--space-xl); display: flex; align-items: center; justify-content: space-between; }
	.browse-controls { display: flex; gap: var(--space-md); align-items: center; }
	.browse-tabs { display: flex; gap: 4px; background: var(--bg); border-radius: var(--radius-md); padding: 3px; }
	.browse-tab { padding: var(--space-xs) var(--space-md); border-radius: var(--radius-sm); font-size: 0.8125rem; font-weight: 600; color: var(--text-muted); border: none; background: none; cursor: pointer; transition: all var(--transition); }
	.browse-tab.active { background: var(--surface); color: var(--text); box-shadow: var(--shadow-sm); }

	.card-list { padding: 0 var(--space-xl) var(--space-xl); display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: var(--space-md); }
	.card-preview { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: var(--space-md); cursor: pointer; transition: all var(--transition); }
	.card-preview:hover { border-color: var(--primary-light); box-shadow: var(--shadow-md); }
	.card-preview-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: var(--space-sm); }
	.preview-word { font-weight: 700; font-size: 1rem; }
	.preview-meaning { font-size: 0.8125rem; color: var(--text-secondary); margin-bottom: var(--space-sm); }
	.card-preview-footer { display: flex; align-items: center; justify-content: space-between; font-size: 0.75rem; color: var(--text-muted); }
	.srs-indicator { display: flex; gap: 3px; }
	.srs-dot { width: 8px; height: 8px; border-radius: var(--radius-full); background: var(--border-light); }
	.srs-dot.filled { background: var(--success); }
</style>
