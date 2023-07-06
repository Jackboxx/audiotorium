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
	<div class="grid h-24 w-full grid-rows-1 gap-2 overflow-x-scroll bg-red-900 lg:h-40">
		{#each sources as source, index}
			<div
				class={`w-1/6 rounded ${
					activeSource === index ? 'bg-blue-400' : 'bg-red-400'
				} p-2 text-lg lg:text-2xl`}
				role="button"
				tabindex="0"
				on:click={() => (activeSource = index)}
				on:keydown={undefined}
			>
				{source}
			</div>
		{/each}
		<div
			class="w-1/6 p-2"
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
