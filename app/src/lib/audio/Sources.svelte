<script lang="ts">
	import { onMount } from 'svelte';
	import type {
		AddSourceMsg,
		ReadQueueItemsMsg,
		SetActiveSourceMsg
	} from '../../schema/messages/queueMessages';
	import type { ResponseHandler } from '../../schema/messages/response';
	import Queue from './Queue.svelte';
	import type {
		AddQueueItemResponse,
		MoveQueueItemResponse,
		ReadQueueItemsResponse,
		SetActiveSourceResponse
	} from '../../schema/messages/queueResponses';

	export const getActiveSource = () => activeSource;

	export let handlers: ResponseHandler[];
	export let sendWsMsg: <T>(msg: [string, T] | [string]) => void;

	export let sources: string[];

	let activeSource: string;
	let queue: string[] = [];

	onMount(() => {
		handlers.push([
			'SET_ACTIVE_SOURCE_RESPONSE',
			(data: SetActiveSourceResponse) => {
				activeSource = data.sourceName;
			}
		]);

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

		handlers.push([
			'MOVE_QUEUE_ITEM_RESPONSE',
			(data: MoveQueueItemResponse) => {
				queue = data.queue;
			}
		]);

		handlers = handlers;
	});

	const addSource = (sourceName: string) => {
		const msg: AddSourceMsg = ['ADD_SOURCE', { sourceName }];
		sendWsMsg(msg);
	};

	const readCurrentQueue = (activeSource: string) => {
		if (!activeSource) {
			return;
		}

		const msg: ReadQueueItemsMsg = ['READ_QUEUE_ITEMS'];
		sendWsMsg(msg);
	};

	const setActiceSource = (sourceName: string) => {
		const msg: SetActiveSourceMsg = ['SET_ACTIVE_SOURCE', { sourceName }];
		sendWsMsg(msg);
	};

	$: readCurrentQueue(activeSource);
</script>

<div class="flex w-full flex-col lg:h-screen">
	<div class="justify-left flex w-full items-start p-2">
		<div class="scrollbar-hide flex items-center gap-1 overflow-x-scroll lg:gap-2">
			{#each sources as source}
				<div
					class={`${
						activeSource === source
							? 'border-indigo-800'
							: 'border-transparent'
					} min-w-[120px] max-w-[250px] truncate rounded border-[2px]
                    bg-zinc-800 p-2 text-center text-lg lg:min-w-[180px] lg:max-w-[350px] lg:text-2xl`}
					role="button"
					tabindex="0"
					on:click={() => setActiceSource(source)}
					on:keydown={undefined}
				>
					{source}
				</div>
			{/each}
			<div
				class="flex h-full cursor-pointer select-none items-center text-center"
				role="button"
				tabindex="0"
				on:click={() => addSource('default')}
				on:keydown={undefined}
			>
				<img class="w-[24px] invert lg:w-[32px]" src="/plus-square.svg" alt="+" />
			</div>
		</div>
	</div>
	{#if sources.length > 0}
		<Queue {queue} {sendWsMsg} bind:handlers />
	{:else}
		<div class="text-2xl font-bold text-red-500 lg:text-4xl">Add an audio source</div>
	{/if}
</div>
