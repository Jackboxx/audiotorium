<script lang="ts">
	import type { AudioNodeHealth } from '$api/AudioNodeHealth';
	import type { AudioNodeInfo } from '$api/AudioNodeInfo';
	import type { BrainSessionWsResponse } from '$api/BrainSessionWsResponse';
	import { onMount } from 'svelte';

	let nodeInfo: AudioNodeInfo[] = [];

	onMount(() => {
		const socket = new WebSocket(
			'ws://127.0.0.1:50051/streams/brain?wanted_info=NODE_INFO'
		);

		socket.addEventListener('message', (event) => {
			try {
				const resp = JSON.parse(event.data) as BrainSessionWsResponse;
				if (resp.SESSION_CONNECTED_RESPONSE !== undefined) {
					nodeInfo = (resp.SESSION_CONNECTED_RESPONSE.node_info ?? []).toSorted(
						(a, b) =>
							a.human_readable_name.localeCompare(b.human_readable_name)
					);
				}
			} catch (err) {
				console.error(err);
			}
		});
	});

	type SimpleHealth = 'good' | 'mild' | 'poor';

	function getSimpleHealth(health: AudioNodeHealth): SimpleHealth {
		if (health === 'good') {
			return 'good';
		} else if ('mild' in health) {
			return 'mild';
		} else {
			return 'poor';
		}
	}

	function getHealthColor(health: SimpleHealth) {
		if (health === 'good') {
			return 'bg-neutral-800 border-neutral-400';
		} else if (health === 'mild') {
			return 'bg-neutral-800  border-orange-600 text-orange-400';
		} else {
			return 'bg-red-900 border-red-500 text-black';
		}
	}
</script>

<div
	style="min-height: 100vh; min-height: -webkit-fill-available;"
	class="h-full bg-neutral-900 p-4"
>
	<div class="grid grid-cols-1 gap-6 sm:grid-cols-2 lg:grid-cols-4 2xl:grid-cols-6">
		{#each nodeInfo as info}
			<div
				class={`flex items-center justify-center border border-solid bg-neutral-800
                 ${getHealthColor(getSimpleHealth(info.health))} p-8 lg:p-12`}
				role="cell"
				tabindex="0"
				on:keydown={undefined}
				on:click={() => {
					if (getSimpleHealth(info.health) !== 'poor') {
						window.location.href = `/queue/${info.source_name}`;
					}
				}}
			>
				<span class="text-lg font-bold sm:text-xl 2xl:text-2xl">
					{info.human_readable_name}
				</span>
			</div>
		{/each}
	</div>
</div>
