import type { PageLoad } from './$types';

export const load: PageLoad = ({ params }: { params: { node: string } }) => {
	return { node: params.node };
};
