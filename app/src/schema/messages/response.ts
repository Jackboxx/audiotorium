export type ResponseHandler = [string, (data: any) => void];

export function handleResponse(response: any, handlers: ResponseHandler[]) {
	let data = JSON.parse(response);

	for (const handler of handlers) {
		let key = handler[0];
		let handle = handler[1];

		if (data[key] !== undefined) {
			handle(data[key]);
		}
	}
}
