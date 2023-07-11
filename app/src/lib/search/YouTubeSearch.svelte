<script lang="ts">
	import SearchBar from './SearchBar.svelte';
	import {
		YouTubeSearchResultSchema,
		searchForVideos,
		type YouTubeVideo,
		getBestThumbNail
	} from '../../schema/video';

	export let onSearchResultClick: (video: YouTubeVideo) => void;

	let searchText = '';
	let searchResults: YouTubeVideo[] = [];

	const onConfirm = async () => {
		const res = await searchForVideos(searchText);

		try {
			searchResults = YouTubeSearchResultSchema.parse(await res.json()).items;
		} catch (err) {
			console.error(err);
		}
	};
</script>

<div class="sticky top-0 h-full w-full">
	<div class="flex w-full justify-center bg-gray-100 py-4 dark:bg-zinc-900">
		<SearchBar bind:searchText {onConfirm} />
	</div>
</div>
{#each searchResults as video}
	<div
		class="my-4 grid aspect-video w-full grid-cols-2 gap-4"
		role="button"
		tabindex="0"
		on:click={() => onSearchResultClick(video)}
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
