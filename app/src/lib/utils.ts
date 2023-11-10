import type { AudioNodeCommand } from '$api/AudioNodeCommand';

export function unixMillisToMinutesString(millis: number): string {
	const total_seconds = Math.trunc(millis / 1000);
	const seconds = total_seconds % 60;
	const minutes = Math.trunc(total_seconds / 60);

	const str_seconds = seconds < 10 ? `0${seconds}` : `${seconds}`;

	return `${minutes}:${str_seconds}`;
}

export async function sendCommandWithTimeout(
	cmd: AudioNodeCommand,
	nodeName: string,
	timeoutMs: number,
	onTimeout: () => void
) {
	const timeoutIdentifier = {};

	try {
		await new Promise(async (res, rej) => {
			setTimeout(() => rej(timeoutIdentifier), timeoutMs);
			res(
				await fetch(`${import.meta.env.VITE_API_PREFIX}/commands/node/${nodeName}`, {
					method: 'POST',
					body: JSON.stringify(cmd),
					headers: { 'Content-Type': 'application/json' }
				})
			);
		});
	} catch (e) {
		if (e === timeoutIdentifier) {
			onTimeout();
		}
	}
}
