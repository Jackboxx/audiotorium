<script lang="ts">
	import { PUBLIC_API_URL_WS } from '$env/static/public';
	import YouTubeSearch from '$lib/search/YouTubeSearch.svelte';
	import { onMount } from 'svelte';
	import { getVideoUrl, type YouTubeVideo } from '../../schema/video';
	import { msgString } from '../../schema/messages/message';
	import { handleResponse, type ResponseHandler } from '../../schema/messages/response';
	import type { AddQueueItemMsg } from '../../schema/messages/queueMessages';
	import Sources from '$lib/audio/Sources.svelte';
	import type {
		AddSourceResponse,
		SessionConnectedResponse
	} from '../../schema/messages/queueResponses';

	let websocket: WebSocket;
	let handlers: ResponseHandler[] = [
		[
			'SESSION_CONNECTED_RESPONSE',
			(data: SessionConnectedResponse) => {
				sources = data.sources;
			}
		],
		[
			'ADD_SOURCE_RESPONSE',
			(data: AddSourceResponse) => {
				sources = data.sources;
			}
		]
	];

	let getActiveSource: () => string;

	let sources: string[] = [];

	onMount(() => {
		websocket = new WebSocket(`${PUBLIC_API_URL_WS}/queue`);
	});

	const sendWsMsg = <T>(msg: [string, T] | [string]) => {
		websocket.send(msgString(msg));
	};

	const onSearchResultClick = (video: YouTubeVideo) => {
		const sourceName = getActiveSource();
		if (!sourceName) {
			return;
		}

		const msg: AddQueueItemMsg = [
			'ADD_QUEUE_ITEM',
			{ title: video.snippet.title, url: getVideoUrl(video) }
		];

		sendWsMsg(msg);
	};

	$: websocket &&
		(websocket.onmessage = (event) => {
			handleResponse(event.data, handlers);
		});
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
		<Sources {sources} bind:getActiveSource bind:handlers {sendWsMsg} />
	</div>
</div>
