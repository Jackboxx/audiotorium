<script lang="ts">
	import { onMount } from 'svelte';
	import ActiveAudio from '$lib/queue/ActiveAudio.svelte';
	import type { PageData } from './$types';
	import type { AudioMetaData } from '$api/AudioMetaData';
	import type { NodeSessionWsResponse } from '$api/NodeSessionWsResponse';
	import type { AudioNodeInfoStreamMessage } from '$api/AudioNodeInfoStreamMessage';
	import type { AudioNodeHealth } from '$api/AudioNodeHealth';
	import type { AudioStateInfo } from '$api/AudioStateInfo';

	export let data: PageData;

	let humanReadableName = '---';
	let queue: AudioMetaData[] = [];
	let health: AudioNodeHealth = 'good';
	let audioStateInfo: AudioStateInfo | undefined = undefined;

	onMount(() => {
		const url = new URLSearchParams(window.location.search);
		humanReadableName = decodeURIComponent(url.get('human_readable_name') ?? '---');

		const socket = new WebSocket(
			`ws://127.0.0.1:50051/streams/node/${data.node}?wanted_info=QUEUE,HEALTH,AUDIO_STATE_INFO`
		);

		socket.addEventListener('message', (event) => {
			try {
				const resp = JSON.parse(event.data);
				const connection_resp = resp as NodeSessionWsResponse;

				if ('SESSION_CONNECTED_RESPONSE' in connection_resp) {
					queue = connection_resp.SESSION_CONNECTED_RESPONSE.queue ?? [];
				}

				const stream_resp = resp as AudioNodeInfoStreamMessage;

				if ('QUEUE' in stream_resp) {
					queue = stream_resp.QUEUE ?? [];
				}

				if ('AUDIO_STATE_INFO' in stream_resp) {
					audioStateInfo = stream_resp.AUDIO_STATE_INFO;
				}
			} catch (err) {
				console.error(err);
			}
		});
	});
</script>

<div class="flex h-full flex-col gap-8 p-4">
	<span class="text-lg font-bold sm:text-xl 2xl:text-2xl"> {humanReadableName}</span>

	<div class="flex flex-grow flex-col gap-2">
		{#each queue as audioDataEntry}
			<div>
				{audioDataEntry.name}
			</div>
		{/each}
	</div>

	<ActiveAudio
		nodeName={data.node}
		playbackInfo={audioStateInfo?.processorInfo}
		activeAudio={audioStateInfo
			? queue[audioStateInfo.playbackInfo.currentHeadIndex]
			: undefined}
	/>
</div>
