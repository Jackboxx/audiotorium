<script lang="ts">
	export let color = '#3730a3';
	export let duration = '0.75s';
	export let size = '60';
	export let visible = true;
</script>

<div
	class={`${visible ? '' : 'opacity-0'} circle`}
	style="--size: {size}px; --color: {color}; --duration: {duration}"
/>

<style>
	.circle {
		height: var(--size);
		width: var(--size);
		border-color: var(--color) transparent var(--color) var(--color);
		border-width: calc(var(--size) / 15);
		border-style: solid;
		border-image: initial;
		border-radius: 50%;
		animation: var(--duration) linear 0s infinite normal none running rotate;
	}

	@keyframes rotate {
		0% {
			transform: rotate(0);
		}
		100% {
			transform: rotate(360deg);
		}
	}
</style>
