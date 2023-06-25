<script lang="ts">
	import SearchBar from '../../lib/SearchBar.svelte';
	import {
		YouTubeSearchResultSchema,
		getVideoUrl,
		searchForVideos,
		type YouTubeVideo,
		getBestThumbNail
	} from '../../schema/video';

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

<div class="flex min-h-screen w-full flex-col items-center bg-gray-100 dark:bg-zinc-900">
	<SearchBar bind:searchText {onConfirm} />
	{#each searchResults as video}
		<a href={getVideoUrl(video)} class="my-4"
			><img src={getBestThumbNail(video.snippet.thumbnails).url} alt="" /></a
		>
	{/each}
</div>
