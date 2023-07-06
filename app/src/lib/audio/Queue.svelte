<script lang="ts">
	import Banner from '$lib/banner.svelte';
	import { flip } from 'svelte/animate';
	import { dndzone, type DndEvent } from 'svelte-dnd-action';
	import type { ResponseHandler } from '../../schema/messages/response';
	import { onMount } from 'svelte';
	import type { SendClientQueueInfoResponse } from '../../schema/messages/queueResponses';

	export let handlers: ResponseHandler[];
	export let sendWsMsg: <T>(msg: [string, T]) => void;

	export let queue: string[];
	export let activeSource: string;

	let items: { name: string; id: number }[] = [];
	let current_index = 0;

	const flipDurationMs = 150;

	onMount(() => {
		handlers.push([
			'SEND_CLIENT_QUEUE_INFO_RESPONSE',
			(data: SendClientQueueInfoResponse) => {
				current_index = data.info.current_head_index;
			}
		]);

		handlers = handlers;
	});

	const handleDndConsider = (
		e: CustomEvent<DndEvent<{ name: string; id: number }>>
	) => {
		items = e.detail.items;
	};
	const handleDndFinalize = (
		e: CustomEvent<DndEvent<{ name: string; id: number }>>
	) => {
		items = e.detail.items;
		console.log('todo: send move msg');
	};

	$: items = queue.map((item, index) => ({ name: item, id: index }));
</script>

<div class="w-full">
	<Banner text="Queue" />
</div>

<section
	class="h-full w-full overflow-y-scroll"
	use:dndzone={{ items, flipDurationMs }}
	on:consider={handleDndConsider}
	on:finalize={handleDndFinalize}
>
	{#each items as item (item.id)}
		<div
			class={`m-2 h-1/6 truncate rounded-lg p-2 ${
				current_index == item.id
					? 'bg-sky-400 dark:bg-indigo-500'
					: 'bg-sky-200 dark:bg-indigo-300'
			} text-center align-middle text-lg
                    lg:text-2xl`}
			animate:flip={{ duration: flipDurationMs }}
		>
			{item.name}
		</div>
	{/each}
</section>
