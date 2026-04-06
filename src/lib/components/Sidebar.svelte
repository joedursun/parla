<script lang="ts">
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { store } from '$lib/stores.svelte';
	import { renameConversation, deleteConversation } from '$lib/conversation';

	const userProfile = $derived(store.userProfile);
	const recentConversations = $derived(store.recentConversations);
	const flashcardsDueCount = $derived(store.flashcardsDueCount);

	const pathname = $derived(page.url.pathname);

	// ── Inline editing state ────────────────────────────────────────────
	let editingId: string | null = $state(null);
	let editValue = $state('');

	function startEditing(conv: { id: string; title: string }, e: MouseEvent) {
		e.preventDefault();
		e.stopPropagation();
		editingId = conv.id;
		editValue = conv.title;
	}

	async function commitEdit() {
		if (editingId && editValue.trim()) {
			try {
				await renameConversation(parseInt(editingId, 10), editValue.trim());
			} catch (err) {
				console.error('Rename failed:', err);
			}
		}
		editingId = null;
	}

	function cancelEdit() {
		editingId = null;
	}

	function handleEditKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') {
			e.preventDefault();
			commitEdit();
		} else if (e.key === 'Escape') {
			cancelEdit();
		}
	}

	async function handleDelete(conv: { id: string; title: string }, e: MouseEvent) {
		e.preventDefault();
		e.stopPropagation();
		try {
			await deleteConversation(parseInt(conv.id, 10));
			// If we were viewing this conversation, navigate away.
			const currentId = page.url.searchParams.get('id');
			if (currentId === conv.id) {
				goto('/conversation');
			}
		} catch (err) {
			console.error('Delete failed:', err);
		}
	}
</script>

<nav class="sidebar">
	<a class="sidebar-logo" href="/">
		<div class="logo-icon">P</div>
		Parla
	</a>

	<a class="nav-item" class:active={pathname === '/'} href="/">
		<span class="nav-icon">&#x2302;</span>
		Dashboard
	</a>
	<a class="nav-item" class:active={pathname.startsWith('/conversation')} href="/conversation">
		<span class="nav-icon">&#x1F4AC;</span>
		Conversations
	</a>
	<a class="nav-item" class:active={pathname.startsWith('/flashcards')} href="/flashcards">
		<span class="nav-icon">&#x1F0CF;</span>
		Flashcards
		{#if flashcardsDueCount > 0}
			<span class="badge">{flashcardsDueCount}</span>
		{/if}
	</a>
	<a class="nav-item" class:active={pathname.startsWith('/progress')} href="/progress">
		<span class="nav-icon">&#x1F4CA;</span>
		Progress
	</a>

	{#if recentConversations.length > 0}
		<div class="nav-section">
			<div class="nav-section-title">Recent Conversations</div>
			{#each recentConversations as conv}
				{#if editingId === conv.id}
					<!-- svelte-ignore a11y_autofocus -->
				<div class="nav-item editing">
						<input
							class="edit-input"
							type="text"
							bind:value={editValue}
							onkeydown={handleEditKeydown}
							onblur={commitEdit}
							autofocus
						/>
					</div>
				{:else}
					<a class="nav-item conv-item" href="/conversation?id={conv.id}">
						<span class="nav-icon">&#x1F4AC;</span>
						<span class="truncate">{conv.title}</span>
						<span class="conv-actions">
							<button
								class="action-btn"
								title="Rename"
								onclick={(e) => startEditing(conv, e)}
							>&#x270E;</button>
							<button
								class="action-btn delete"
								title="Delete"
								onclick={(e) => handleDelete(conv, e)}
							>&#x2715;</button>
						</span>
					</a>
				{/if}
			{/each}
		</div>
	{/if}

	<div class="sidebar-footer">
		<a class="nav-item" href="/">
			<span class="nav-icon">&#x2699;</span>
			Settings
		</a>
		{#if userProfile}
			<div class="user-card">
				<div class="user-avatar">{userProfile.name.charAt(0).toUpperCase()}</div>
				<div class="user-info">
					<div class="user-name">{userProfile.name}</div>
					<div class="user-level">Learning {userProfile.targetLanguage}</div>
				</div>
			</div>
		{/if}
	</div>
</nav>

<style>
	.sidebar {
		width: 240px;
		background: var(--surface);
		border-right: 1px solid var(--border);
		display: flex;
		flex-direction: column;
		padding: var(--space-md);
		gap: var(--space-xs);
		flex-shrink: 0;
	}

	.sidebar-logo {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		padding: var(--space-sm) var(--space-sm) var(--space-lg);
		font-weight: 700;
		font-size: 1.25rem;
		color: var(--primary);
		text-decoration: none;
	}

	.logo-icon {
		width: 32px;
		height: 32px;
		background: var(--primary);
		border-radius: var(--radius-md);
		display: flex;
		align-items: center;
		justify-content: center;
		color: white;
		font-size: 1rem;
	}

	.nav-section {
		margin-top: var(--space-lg);
	}

	.nav-section-title {
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.08em;
		color: var(--text-muted);
		padding: 0 var(--space-sm);
		margin-bottom: var(--space-xs);
	}

	.nav-item {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		padding: var(--space-sm) var(--space-md);
		border-radius: var(--radius-md);
		color: var(--text-secondary);
		text-decoration: none;
		font-size: 0.9375rem;
		font-weight: 500;
		transition: all var(--transition);
		cursor: pointer;
		position: relative;
	}

	.nav-item:hover {
		background: var(--surface-hover);
		color: var(--text);
	}

	.nav-item.active {
		background: var(--primary-subtle);
		color: var(--primary);
		font-weight: 600;
	}

	.nav-icon {
		width: 20px;
		height: 20px;
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 1rem;
		flex-shrink: 0;
	}

	.badge {
		margin-left: auto;
		background: var(--secondary);
		color: white;
		font-size: 0.6875rem;
		font-weight: 700;
		padding: 1px 7px;
		border-radius: var(--radius-full);
		min-width: 20px;
		text-align: center;
	}

	/* ── Conversation item actions ───────────────────────────────── */
	.conv-actions {
		display: none;
		margin-left: auto;
		gap: 2px;
		flex-shrink: 0;
	}
	.conv-item:hover .conv-actions {
		display: flex;
	}
	.action-btn {
		width: 22px;
		height: 22px;
		border: none;
		background: none;
		color: var(--text-muted);
		font-size: 0.75rem;
		cursor: pointer;
		border-radius: var(--radius-sm);
		display: flex;
		align-items: center;
		justify-content: center;
		transition: all var(--transition);
	}
	.action-btn:hover {
		background: var(--bg);
		color: var(--text);
	}
	.action-btn.delete:hover {
		color: var(--danger);
	}

	/* ── Inline editing ──────────────────────────────────────────── */
	.nav-item.editing {
		padding: var(--space-xs) var(--space-sm);
	}
	.edit-input {
		width: 100%;
		border: 1px solid var(--primary);
		background: var(--bg);
		color: var(--text);
		font-family: var(--font);
		font-size: 0.875rem;
		padding: var(--space-xs) var(--space-sm);
		border-radius: var(--radius-sm);
		outline: none;
	}

	.sidebar-footer {
		margin-top: auto;
		padding-top: var(--space-md);
		border-top: 1px solid var(--border-light);
	}

	.user-card {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		padding: var(--space-sm);
		border-radius: var(--radius-md);
		cursor: pointer;
	}

	.user-card:hover {
		background: var(--surface-hover);
	}

	.user-avatar {
		width: 36px;
		height: 36px;
		border-radius: var(--radius-full);
		background: var(--primary-subtle);
		color: var(--primary);
		display: flex;
		align-items: center;
		justify-content: center;
		font-weight: 700;
		font-size: 0.875rem;
	}

	.user-info {
		flex: 1;
		min-width: 0;
	}

	.user-name {
		font-weight: 600;
		font-size: 0.875rem;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.user-level {
		font-size: 0.75rem;
		color: var(--text-muted);
	}

	.truncate {
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
</style>
