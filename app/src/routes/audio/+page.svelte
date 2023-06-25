<script>
	import SearchBar from '../../lib/SearchBar.svelte';
	let searchText = '';
</script>

<div class="flex min-h-screen w-full flex-col items-center bg-gray-100 dark:bg-zinc-900">
	<SearchBar
		bind:searchText
		onConfirm={() => console.log(`current text: ${searchText}`)}
	/>
</div>
