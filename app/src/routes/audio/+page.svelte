<script lang="ts">
	import { PUBLIC_API_URL_WS } from '$env/static/public';
	import Banner from '$lib/banner.svelte';
	import YouTubeSearch from '$lib/search/YouTubeSearch.svelte';
	import { onMount } from 'svelte';
	import { getVideoUrl, type YouTubeVideo } from '../../schema/video';
	import { msgString } from '../../schema/messages/message';
	import { handleResponse } from '../../schema/messages/response';
	import type {
		AddQueueItemMsg,
		AddSourceMsg
	} from '../../schema/messages/queueMessages';

	let websocket: WebSocket;

	let queue = [] as string[];
	let current_index = 0;

	const handlers = [
		[
			'SEND_CLIENT_QUEUE_INFO_RESPONSE',
			(data: { info: { current_head_index: number } }) => {
				current_index = data.info.current_head_index;
			}
		],
		[
			'ADD_QUEUE_ITEM_RESPONSE',
			(data: { queue: string[] }) => {
				queue = data.queue;
			}
		]
	];

	onMount(() => {
		websocket = new WebSocket(`${PUBLIC_API_URL_WS}/queue`);
		websocket.onmessage = (event) => {
			handleResponse(event.data, handlers);
		};

		websocket.onopen = (_) => {
			const msg: AddSourceMsg = ['ADD_SOURCE', { sourceName: 'default' }];
			websocket.send(msgString(msg));
		};
	});

	const onSearchResultClick = (video: YouTubeVideo) => {
		const msg: AddQueueItemMsg = [
			'ADD_QUEUE_ITEM',
			{ sourceName: 'default', title: video.snippet.title, url: getVideoUrl(video) }
		];

		websocket.send(msgString(msg));
	};
</script>

<div
	class="grid h-screen w-full grid-rows-5
    bg-gray-100 dark:bg-zinc-900 lg:grid-cols-2 lg:grid-rows-1"
>
	<YouTubeSearch {onSearchResultClick} />

	<div
		class="row-span-2 flex flex-col items-center gap-2 lg:row-span-1 lg:border-l-[1px]
        lg:border-l-black lg:dark:border-l-neutral-300"
	>
		<div class="w-full">
			<Banner text="Playing" />
		</div>
		<div class="text-lg">{queue.at(0)}</div>

		<div class="w-full">
			<Banner text="Next" />
		</div>
		<div class="flex w-full flex-col gap-1 overflow-y-scroll">
			{#each queue.splice(1) as item}
				<div class="text-lg">{item}</div>
			{/each}
		</div>
	</div>
</div>
