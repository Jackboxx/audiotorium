<script lang="ts">
	import SearchBar from './SearchBar.svelte';
	import {
		YouTubeSearchResultSchema,
		searchForVideos,
		type YouTubeVideo,
		getBestThumbNail
	} from '../../schema/video';
	import { onMount } from 'svelte';
	import type { ResponseHandler } from '../../schema/messages/response';

	export let onSearchResultClick: (video: YouTubeVideo) => void;
	export let handlers: ResponseHandler[];

	let searchText = '';
	let searchResults: YouTubeVideo[] = [];

	let downloadingAudio = false;

	onMount(() => {
		handlers.push([
			'STARTED_DOWNLOADING_AUDIO',
			(_) => {
				downloadingAudio = true;
			}
		]);

		handlers.push([
			'FINISHED_DOWNLOADING_AUDIO',
			(_) => {
				downloadingAudio = false;
			}
		]);

		handlers = handlers;
	});

	const onConfirm = async () => {
		const res = await searchForVideos(searchText);

		try {
			searchResults = YouTubeSearchResultSchema.parse(await res.json()).items;
		} catch (err) {
			console.error(err);
		}
	};
</script>

{#if downloadingAudio}
	<div class="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 transform">
		Downloading
	</div>
{/if}

<div class="sticky top-0 h-full w-full">
	<div class="flex w-full justify-center bg-zinc-900 py-4">
		<SearchBar bind:searchText {onConfirm} />
	</div>
</div>

{#each searchResults as video}
	<div
		class="my-4 grid aspect-video w-full grid-cols-2 gap-4"
		role="button"
		tabindex="0"
		on:click={() => {
			if (!downloadingAudio) {
				onSearchResultClick(video);
			}
		}}
		on:keydown={() => {}}
	>
		<div>
			<img
				class="h-full w-full rounded object-cover"
				src={getBestThumbNail(video.snippet.thumbnails).url}
				alt={video.snippet.title}
			/>
		</div>
		<div class="flex flex-col gap-2">
			<span class="text-lg lg:text-2xl">
				{video.snippet.title}
			</span>
			<span class="lg:text-md text-sm"> Supper cool channel name + img</span>
			<span class="lg:text-md text-sm"> 5min </span>
		</div>
	</div>
{/each}
