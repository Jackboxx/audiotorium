import type { AudioNodeCommand } from '$api/AudioNodeCommand';

export const API_PREFIX = 'http://127.0.0.1:50051';
export const WS_PREFIX = 'ws://127.0.0.1:50051';

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
				await fetch(`${API_PREFIX}/commands/node/${nodeName}`, {
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
