<script>
	import '../app.css';
	import { SvelteToast } from '@zerodevx/svelte-toast';

	const options = {
		duration: 3000,
		pausable: true,
		dismissable: true,
		intro: { x: 256 },
		classes: ['log']
	};
</script>

<slot />
<div class="text-lg font-bold text-white">
	<SvelteToast {options} />
</div>

<style>
	:global(.log.error) {
		--toastBackground: #ef4444;
		--toastBarBackground: #1c1917;
	}
</style>
