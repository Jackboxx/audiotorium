<script lang="ts">
	export let onDblClick: () => void;
	export let title: string;
	export let duration: number;
</script>

<div
	class="m-2 h-[80px] rounded border-[2px] border-transparent bg-gradient-to-r from-gray-200 to-gray-300
        p-2 shadow-md dark:from-neutral-700 dark:to-neutral-800 lg:h-[130px]"
	on:dblclick={onDblClick}
	on:keydown={undefined}
	role="button"
	tabindex="0"
>
	<div class="truncate text-lg lg:text-2xl">{title}</div>
	<div class="truncate text-sm lg:text-lg">{duration}min</div>
</div>
