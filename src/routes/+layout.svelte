<script lang="ts">
	import '../app.css';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { onMount, onDestroy } from 'svelte';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import { getProfile, getRecentConversations } from '$lib/conversation';
	import {
		setUserProfile,
		setFlashcardsDueCount,
		setRecentVocabulary,
		setRecentConversations,
		type VocabWord,
		type RecentConversation,
	} from '$lib/stores.svelte';

	let { children } = $props();
	let ready = $state(false);
	let unlisteners: UnlistenFn[] = [];

	const isOnboarding = $derived(page.url.pathname === '/onboarding');

	onMount(async () => {
		try {
			const profile = await getProfile();
			if (profile) {
				setUserProfile({
					name: 'Learner',
					nativeLanguage: profile.native_language,
					targetLanguage: profile.target_language,
					level: profile.cefr_level,
					levelLabel: profile.cefr_level,
					goals: profile.goals,
				});
			} else if (page.url.pathname !== '/onboarding') {
				goto('/onboarding');
			}
		} catch (e) {
			console.error('Failed to load profile:', e);
		}
		ready = true;

		// Load recent conversations from DB.
		try {
			const convs = await getRecentConversations();
			setRecentConversations(convs);
		} catch {}

		// Listen for store-update events from the Rust backend.
		unlisteners.push(
			await listen<number>('flashcards-due-count', (e) => {
				setFlashcardsDueCount(e.payload);
			}),
		);
		unlisteners.push(
			await listen<VocabWord[]>('recent-vocabulary', (e) => {
				setRecentVocabulary(e.payload);
			}),
		);
		unlisteners.push(
			await listen<RecentConversation[]>('recent-conversations', (e) => {
				setRecentConversations(e.payload);
			}),
		);
	});

	onDestroy(() => {
		for (const un of unlisteners) un();
	});
</script>

{#if isOnboarding}
	{@render children()}
{:else}
	<div class="app-layout">
		<Sidebar />
		<main class="main-content">
			{@render children()}
		</main>
	</div>
{/if}

<style>
	.app-layout {
		display: flex;
		height: 100vh;
		overflow: hidden;
	}

	.main-content {
		flex: 1;
		overflow-y: auto;
		display: flex;
		flex-direction: column;
	}
</style>
