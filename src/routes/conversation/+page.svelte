<script lang="ts">
	type Message = {
		role: 'tutor' | 'student';
		target: string;
		translation: string;
		correction?: { wrong: string; right: string; alt?: string; explain: string };
		vocab?: { word: string; meaning: string };
	};

	const messages: Message[] = [
		{
			role: 'tutor',
			target: '\u00A1Hola! Hoy vamos a practicar como pedir comida en un restaurante. Imagina que estamos en un cafe en Madrid. Yo soy el mesero. \u00BFQue te gustaria ordenar?',
			translation: "Hi! Today we're going to practice ordering food at a restaurant. Imagine we're at a cafe in Madrid. I'm the waiter. What would you like to order?",
		},
		{
			role: 'student',
			target: 'Hola! Me gustaria un cafe con leche, por favor.',
			translation: 'Hi! I would like a coffee with milk, please.',
		},
		{
			role: 'tutor',
			target: '\u00A1Muy bien! Un cafe con leche, excelente eleccion. \u00BFY quieres algo para comer? Tenemos churros, tostadas, y tortilla espanola.',
			translation: 'Very good! A coffee with milk, excellent choice. And do you want something to eat? We have churros, toast, and Spanish omelette.',
			vocab: { word: 'tortilla espanola', meaning: 'Spanish omelette (potato & egg)' },
		},
		{
			role: 'student',
			target: 'Si, yo quiero los churros. \u00BFCuanto es?',
			translation: 'Yes, I want the churros. How much is it?',
			correction: {
				wrong: '\u00BFCuanto es?',
				right: '\u00BFCuanto cuesta?',
				alt: '\u00BFCuanto cuestan?',
				explain: 'When asking about prices, use costar (to cost). Since churros is plural, you\'d say \u00BFcuanto cuestan? \u2014 "how much do they cost?"'
			},
		},
		{
			role: 'tutor',
			target: '\u00A1Buena eleccion! Los churros cuestan tres euros con cincuenta. \u00BFQuieres algo mas, o te traigo la cuenta?',
			translation: 'Good choice! The churros cost three euros and fifty cents. Would you like anything else, or shall I bring you the bill?',
		},
	];

	const contextVocab = [
		{ word: 'me gustaria', meaning: 'I would like', isNew: false },
		{ word: 'la cuenta', meaning: 'the bill', isNew: false },
		{ word: 'cuanto cuesta', meaning: 'how much does it cost', isNew: true },
		{ word: 'la eleccion', meaning: 'the choice', isNew: true },
		{ word: 'algo mas', meaning: 'anything else', isNew: false },
		{ word: 'tortilla espanola', meaning: 'Spanish omelette', isNew: true },
	];

	const suggestions = [
		{ target: 'No, eso es todo. La cuenta, por favor.', native: "That's all. The bill, please." },
		{ target: '\u00BFTienen algun postre?', native: 'Do you have any desserts?' },
		{ target: 'Me gustaria tambien un vaso de agua.', native: 'I would also like a glass of water.' },
	];
</script>

<div class="conversation-layout">
	<div class="chat-area">
		<div class="chat-header">
			<div class="tutor-avatar-sm">&#x1F393;</div>
			<div class="chat-header-info">
				<div class="chat-header-title">Your Spanish Tutor</div>
				<div class="chat-header-meta">
					<span class="dot"></span>
					Active &middot; Lesson 4: Ordering Food
				</div>
			</div>
			<div class="chat-header-actions">
				<button class="btn btn-ghost btn-icon" title="Toggle translations">Aa</button>
				<button class="btn btn-ghost btn-icon" title="Audio settings">&#x1F50A;</button>
				<button class="btn btn-ghost btn-icon" title="More options">&#x22EE;</button>
			</div>
		</div>

		<div class="lesson-banner">
			<span>&#x1F37D;</span>
			<span>Lesson: Ordering Food &amp; Drinks at a Restaurant</span>
			<div class="progress-mini">
				<span class="text-xs">60%</span>
				<div class="progress-bar" style="width:120px;height:4px;">
					<div class="fill" style="width: 60%"></div>
				</div>
			</div>
		</div>

		<div class="messages">
			{#each messages as msg}
				<div class="message {msg.role}">
					<div class="message-avatar">
						{#if msg.role === 'tutor'}&#x1F393;{:else}J{/if}
					</div>
					<div class="message-content">
						<div class="bubble">
							<span class="target-text">{msg.target}</span>
							<span class="translation">{msg.translation}</span>
						</div>
						{#if msg.role === 'tutor'}
							<div class="message-actions">
								<button class="msg-action-btn">&#x1F50A; Listen</button>
								<button class="msg-action-btn">&#x1F40C; Slow</button>
								<button class="msg-action-btn">&#x1F441; Translation</button>
							</div>
						{/if}
					</div>
				</div>

				{#if msg.correction}
					<div class="correction">
						<span class="correction-icon">&#x1F4A1;</span>
						<div>
							<div>
								<span class="wrong">{msg.correction.wrong}</span> &rarr;
								<span class="right">{msg.correction.right}</span>
								{#if msg.correction.alt}
									or <span class="right">{msg.correction.alt}</span>
								{/if}
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
		</div>

		<div class="chat-input-area">
			<div class="input-row">
				<textarea rows="1" placeholder="Type in Spanish (or English to translate)..."></textarea>
				<div class="input-actions">
					<button class="voice-btn" title="Hold to speak">&#x1F3A4;</button>
					<button class="send-btn" title="Send message">&#x27A4;</button>
				</div>
			</div>
			<div class="input-hint">
				Press Enter to send &middot; Hold the mic button to speak &middot; Type in English and we'll help you translate
			</div>
		</div>
	</div>

	<div class="context-panel">
		<div class="context-section">
			<h4>&#x1F3AF; Lesson Focus</h4>
			<div class="lesson-focus-card">
				<h3>Ordering Food &amp; Drinks</h3>
				<p>Practice ordering at a restaurant, asking about prices, and making polite requests</p>
				<div class="focus-tags">
					<span class="tag tag-primary">Vocabulary</span>
					<span class="tag tag-secondary">Polite forms</span>
					<span class="tag tag-warning">Numbers</span>
				</div>
			</div>
		</div>

		<div class="context-section">
			<h4>&#x1F4D6; Vocabulary This Lesson</h4>
			{#each contextVocab as item}
				<div class="context-vocab-item">
					<div>
						<div class="context-vocab-word">{item.word}</div>
						<div class="context-vocab-meaning">{item.meaning}</div>
					</div>
					{#if item.isNew}
						<span class="new-badge">New</span>
					{/if}
				</div>
			{/each}
		</div>

		<div class="context-section">
			<h4>&#x1F4DD; Grammar Notes</h4>
			<div class="grammar-note">
				<h4>Costar (to cost)</h4>
				<p>Use <code>cuesta</code> for singular, <code>cuestan</code> for plural items.</p>
			</div>
			<div class="grammar-note">
				<h4>Polite Ordering</h4>
				<p><code>Me gustaria</code> (I would like) is more polite than <code>quiero</code> (I want) when ordering.</p>
			</div>
		</div>

		<div class="context-section">
			<h4>&#x1F4AC; Try Saying</h4>
			<div class="suggestion-chips">
				{#each suggestions as s}
					<button class="suggestion-chip">
						<div class="chip-text">{s.target}</div>
						<div class="chip-hint">{s.native}</div>
					</button>
				{/each}
			</div>
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

	.lesson-banner { background: var(--primary-subtle); padding: var(--space-sm) var(--space-lg); display: flex; align-items: center; gap: var(--space-sm); font-size: 0.8125rem; color: var(--primary); font-weight: 500; border-bottom: 1px solid #D4CEF8; }
	.progress-mini { margin-left: auto; display: flex; align-items: center; gap: var(--space-sm); }
	.text-xs { font-size: 0.75rem; }

	.messages { flex: 1; overflow-y: auto; padding: var(--space-lg); display: flex; flex-direction: column; gap: var(--space-md); }

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

	.message-actions { display: flex; gap: 4px; padding: 0 4px; }
	.msg-action-btn { background: none; border: none; cursor: pointer; font-size: 0.8125rem; color: var(--text-muted); padding: 2px 6px; border-radius: var(--radius-sm); transition: all var(--transition); }
	.msg-action-btn:hover { background: var(--surface-hover); color: var(--text); }

	.correction { background: var(--accent-gold-subtle); border: 1px solid #F0D88A; border-radius: var(--radius-md); padding: var(--space-sm) var(--space-md); font-size: 0.8125rem; display: flex; align-items: flex-start; gap: var(--space-sm); max-width: 75%; align-self: flex-start; margin-left: 40px; }
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
	.voice-btn:hover { background: #d4634d; transform: scale(1.05); }
	.send-btn { width: 40px; height: 40px; border-radius: var(--radius-full); border: none; background: var(--primary); color: white; font-size: 1.125rem; cursor: pointer; display: flex; align-items: center; justify-content: center; transition: all var(--transition); }
	.send-btn:hover { background: var(--primary-dark); }
	.input-hint { text-align: center; font-size: 0.75rem; color: var(--text-muted); margin-top: var(--space-xs); }

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
	.grammar-note code { background: var(--primary-subtle); color: var(--primary); padding: 1px 5px; border-radius: 3px; font-family: var(--font); font-size: 0.8125rem; font-weight: 600; }

	.suggestion-chips { display: flex; flex-direction: column; gap: var(--space-xs); }
	.suggestion-chip { background: var(--bg); border: 1px solid var(--border); border-radius: var(--radius-md); padding: var(--space-sm) var(--space-md); font-size: 0.8125rem; cursor: pointer; transition: all var(--transition); text-align: left; }
	.suggestion-chip:hover { border-color: var(--primary); background: var(--primary-subtle); }
	.chip-text { font-weight: 600; color: var(--text); }
	.chip-hint { color: var(--text-muted); font-size: 0.75rem; }
</style>
