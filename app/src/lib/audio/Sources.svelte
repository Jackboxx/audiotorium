<script lang="ts">
	import { onMount } from 'svelte';
	import type {
		AddSourceMsg,
		ReadQueueItemsMsg
	} from '../../schema/messages/queueMessages';
	import type { ResponseHandler } from '../../schema/messages/response';
	import Queue from './Queue.svelte';
	import type {
		AddQueueItemResponse,
		ReadQueueItemsResponse
	} from '../../schema/messages/queueResponses';

	export const getActiveSource = () => sources[activeSource];

	export let handlers: ResponseHandler[];
	export let sendWsMsg: <T>(msg: [string, T]) => void;

	export let sources: string[];

	let activeSource = 0;
	let queue: string[] = [];

	onMount(() => {
		handlers.push([
			'ADD_QUEUE_ITEM_RESPONSE',
			(data: AddQueueItemResponse) => {
				queue = data.queue;
			}
		]);

		handlers.push([
			'READ_QUEUE_ITEMS_RESPONSE',
			(data: ReadQueueItemsResponse) => {
				queue = data.queue;
			}
		]);

		handlers = handlers;
	});

	const addSource = (sourceName: string) => {
		const msg: AddSourceMsg = ['ADD_SOURCE', { sourceName }];
		sendWsMsg(msg);
	};

	const readCurrentQueue = (activeSource: number) => {
		const sourceName = sources[activeSource];
		if (!sourceName) {
			return;
		}

		const msg: ReadQueueItemsMsg = ['READ_QUEUE_ITEMS', { sourceName }];
		sendWsMsg(msg);
	};

	$: sources && readCurrentQueue(activeSource);
	$: readCurrentQueue(activeSource);
</script>

<div class="h-full w-full">
	<div class="justify-left flex w-full items-start p-1">
		<div class="scrollbar-hide flex overflow-x-scroll">
			{#each sources as source, index}
				<div
					class={`max-w-[250px] truncate rounded ${
						activeSource === index
							? 'bg-gray-300 dark:bg-zinc-700'
							: 'bg-gray-300 dark:bg-zinc-700'
					} ml-1 p-2 text-lg lg:text-2xl `}
					role="button"
					tabindex="0"
					on:click={() => (activeSource = index)}
					on:keydown={undefined}
				>
					{source}
				</div>
			{/each}
		</div>
		<div
			class="ml-1 min-w-[40px] cursor-pointer select-none p-2 text-center"
			role="button"
			tabindex="0"
			on:click={() => addSource('default')}
			on:keydown={undefined}
		>
			+
		</div>
	</div>
	{#if sources.length > 0}
		<Queue {queue} {sendWsMsg} activeSource={sources[activeSource]} bind:handlers />
	{:else}
		<div class="text-2xl font-bold text-red-500 lg:text-4xl">Add an audio source</div>
	{/if}
</div>
