<script lang="ts">
	import '../app.css';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import { page } from '$app/state';

	let { children } = $props();

	const isOnboarding = $derived(page.url.pathname === '/onboarding');
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
