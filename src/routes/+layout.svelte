<script lang="ts">
	import '../app.css';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { onMount, onDestroy } from 'svelte';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import { getProfile, getRecentConversations, getLessons } from '$lib/conversation';
	import type { LessonResult } from '$lib/conversation';
	import {
		setUserProfile,
		setFlashcardsDueCount,
		setRecentVocabulary,
		setRecentConversations,
		setLessons,
		type VocabWord,
		type RecentConversation,
		type Lesson,
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

		// Load recent conversations and lessons from DB.
		try {
			const convs = await getRecentConversations();
			setRecentConversations(convs);
		} catch {}
		try {
			const lessons = await getLessons();
			setLessons(mapLessons(lessons));
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
		unlisteners.push(
			await listen<LessonResult[]>('lessons-updated', (e) => {
				setLessons(mapLessons(e.payload));
			}),
		);
	});

	/** Convert backend LessonResult[] to store Lesson[] with computed status. */
	function mapLessons(results: LessonResult[]): Lesson[] {
		// Find the first non-completed lesson to mark as "current".
		const firstActive = results.findIndex(
			(l) => l.status === 'in_progress' || l.status === 'planned',
		);
		return results.map((l, i) => ({
			id: l.id,
			sequenceOrder: l.sequenceOrder,
			title: l.title,
			description: l.description,
			topic: l.topic,
			cefrLevel: l.cefrLevel,
			successRate: l.successRate,
			status:
				l.status === 'completed'
					? ('done' as const)
					: i === firstActive
						? ('current' as const)
						: ('upcoming' as const),
			progress:
				l.status === 'completed'
					? 'Done'
					: l.status === 'in_progress'
						? 'In progress'
						: 'Upcoming',
		}));
	}

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
