export function handleResponse(response: any, handlers: [string, (data: any) => void][]) {
	let data = JSON.parse(response);

	for (const handler of handlers) {
		let key = handler[0];
		let handle = handler[1];

		if (data[key] !== undefined) {
			handle(data[key]);
		}
	}
}
